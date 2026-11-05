//! Running `sh -c` in its own process group: streaming its output, and
//! killing the whole group on timeout or stop.

use crate::domain::event::OutputStream;
use crate::tools::sandbox::Sandbox;
use std::io::BufRead;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const POLL: Duration = Duration::from_millis(50);

pub enum Ending {
    Exited(ExitStatus),
    TimedOut,
    Stopped,
}

pub struct Execution {
    pub stdout: String,
    pub stderr: String,
    pub ending: Ending,
    pub duration_ms: u64,
}

/// What to start and in which world: `workdir` is the writable root, `env`
/// extra variables (bwrap passes its environment through, so they reach the
/// command with and without the sandbox).
pub struct Launch<'a> {
    pub command: &'a str,
    pub workdir: &'a Path,
    pub cwd: &'a Path,
    pub sandbox: &'a Sandbox,
    pub env: &'a [(String, PathBuf)],
}

pub fn execute(
    launch: Launch,
    timeout: Duration,
    stop: &AtomicBool,
    on_output: &mut dyn FnMut(OutputStream, &str),
) -> Result<Execution, String> {
    let Launch {
        command,
        workdir,
        cwd,
        sandbox,
        env,
    } = launch;
    // The cwd must exist: for the chat, this is where its per-session scratch
    // directory is created, on the first command.
    std::fs::create_dir_all(cwd).map_err(|e| format!("create {}: {e}", cwd.display()))?;
    let started = Instant::now();
    let sandbox_on = matches!(sandbox, Sandbox::Bwrap { .. });
    let mut child = sandbox
        .command(command, workdir, cwd)
        .envs(env.iter().map(|(name, value)| (name, value)))
        // Its own process group, so a stop or timeout kills what the shell started too:
        // a surviving grandchild keeps the pipes open. Under bwrap, killing the group
        // kills bwrap; --die-with-parent and the PID namespace then end the rest: when
        // bwrap's init dies, the kernel kills every process in the namespace.
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound && sandbox_on {
                "bwrap is not installed: install bubblewrap, or turn the sandbox off in Settings > Security > Sandbox."
                    .to_string()
            } else {
                format!("Failed to start command: {e}")
            }
        })?;

    let (tx, rx) = mpsc::channel();
    let readers = [
        spawn_reader(
            child.stdout.take().expect("stdout is piped"),
            OutputStream::Stdout,
            tx.clone(),
        ),
        spawn_reader(
            child.stderr.take().expect("stderr is piped"),
            OutputStream::Stderr,
            tx,
        ),
    ];
    let mut out = Execution {
        stdout: String::new(),
        stderr: String::new(),
        ending: Ending::Stopped,
        duration_ms: 0,
    };
    let mut collect = |stream: OutputStream, chunk: String| {
        on_output(stream, &chunk);
        match stream {
            OutputStream::Stdout => out.stdout.push_str(&chunk),
            OutputStream::Stderr => out.stderr.push_str(&chunk),
        }
    };

    let ending = loop {
        // Pipes can close before the process is reaped: drain what is there
        // and keep polling try_wait, so the exit code and the timeout apply.
        while let Ok((stream, chunk)) = rx.try_recv() {
            collect(stream, chunk);
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("Failed to wait for command: {e}"))?
        {
            break Ending::Exited(status);
        }
        let ending = if stop.load(Ordering::Relaxed) {
            Some(Ending::Stopped)
        } else if started.elapsed() >= timeout {
            Some(Ending::TimedOut)
        } else {
            None
        };
        if let Some(ending) = ending {
            kill_group(&mut child);
            let _ = child.wait();
            break ending;
        }
        thread::sleep(POLL);
    };
    while let Ok((stream, chunk)) = rx.recv() {
        collect(stream, chunk);
    }
    for reader in readers {
        let _ = reader.join();
    }
    out.ending = ending;
    out.duration_ms = started.elapsed().as_millis() as u64;
    Ok(out)
}

fn spawn_reader(
    pipe: impl std::io::Read + Send + 'static,
    stream: OutputStream,
    tx: mpsc::Sender<(OutputStream, String)>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || read_pipe_lines(pipe, stream, tx))
}

fn kill_group(child: &mut std::process::Child) {
    // std cannot signal a group; kill(1) can. The group id is the child's pid.
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{}", child.id())])
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
}

fn read_pipe_lines(
    pipe: impl std::io::Read,
    stream: OutputStream,
    tx: mpsc::Sender<(OutputStream, String)>,
) {
    let mut reader = std::io::BufReader::new(pipe);
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            // Lossy: one invalid UTF-8 byte must not cut off the rest of the output.
            Ok(n) if n > 0 => {
                if tx
                    .send((stream, String::from_utf8_lossy(&line).into_owned()))
                    .is_err()
                {
                    break;
                }
            }
            _ => break,
        }
    }
}
