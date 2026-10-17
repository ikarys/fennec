//! A world: a reversible, forkable place where an agent runs.
//!
//! A world is a git worktree on its own branch `world/<id>`, entered only
//! through a bubblewrap sandbox. Inside, anything goes; nothing leaves except
//! through the harness. Checkpoints are commits made by the harness, never by
//! the agent: the project's object store is mounted read-only in the sandbox.
//!
//! The world's files, `.git` included, are written by the agent, so the
//! harness never takes a path or a setting from them: it locates the git
//! store from its own metadata and runs git with hooks disabled.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Hooks would run project code on the host over files the agent wrote.
const NO_HOOKS: [&str; 2] = ["-c", "core.hooksPath=/dev/null"];

const COMMITTER: [&str; 4] = [
    "-c",
    "user.name=fennec-os",
    "-c",
    "user.email=fennec-os@localhost",
];

/// Where worlds live and what the sandbox lets through.
pub struct Worlds {
    pub root: PathBuf,
    /// Hidden in the sandbox: replaced by an empty tmpfs.
    pub home: PathBuf,
    /// Re-exposed read-only inside the hidden home (toolchains).
    pub ro_paths: Vec<PathBuf>,
    /// Address-space cap per sandboxed command, in MiB; `None` is unlimited.
    pub memory_mb: Option<u64>,
}

pub struct World {
    pub id: String,
    pub path: PathBuf,
}

impl Worlds {
    /// `FENNEC_OS_HOME` (default `~/.local/share/fennec-os`) and
    /// `FENNEC_OS_RO` (colon-separated, default the usual toolchain dirs) and
    /// `FENNEC_OS_MEMORY_MB` (default 8192, `0` for no cap).
    pub fn from_env() -> io::Result<Self> {
        let home = PathBuf::from(
            std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME is not set"))?,
        );
        let root = std::env::var_os("FENNEC_OS_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share/fennec-os"));
        let ro_paths = match std::env::var_os("FENNEC_OS_RO") {
            Some(v) => std::env::split_paths(&v).collect(),
            None => [".cargo", ".rustup", ".local/bin"]
                .iter()
                .map(|p| home.join(p))
                .collect(),
        };
        let memory_mb = match std::env::var("FENNEC_OS_MEMORY_MB") {
            Ok(v) => Some(v.parse::<u64>().map_err(io::Error::other)?).filter(|&m| m > 0),
            Err(_) => Some(8192),
        };
        Ok(Self {
            root,
            home,
            ro_paths,
            memory_mb,
        })
    }

    /// Fork a project (any git directory) or an existing world, from its last commit.
    pub fn fork(&self, source: &str) -> io::Result<World> {
        let (common, rev) = match self.open(source) {
            Ok(world) => (self.common_dir(&world)?, format!("world/{}", world.id)),
            Err(_) => {
                let project = Path::new(source);
                let common = git(
                    project,
                    &["rev-parse", "--path-format=absolute", "--git-common-dir"],
                )?;
                (PathBuf::from(common), git(project, &["rev-parse", "HEAD"])?)
            }
        };
        let id = new_id();
        let path = self.root.join("worlds").join(&id);
        let meta = self.root.join("meta");
        std::fs::create_dir_all(&meta)?;
        std::fs::create_dir_all(path.parent().unwrap_or(&self.root))?;
        let mut args = vec![
            "--git-dir".to_string(),
            common.to_string_lossy().into_owned(),
        ];
        args.extend(["worktree", "add", "-q", "-b", &format!("world/{id}")].map(String::from));
        args.extend([path.to_string_lossy().into_owned(), rev]);
        git(
            &self.root,
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
        )?;
        // git names the admin dir after the path's basename; a stale dir of the
        // same name would get a suffix and break the derivation below.
        if !common.join("worktrees").join(&id).is_dir() {
            return Err(io::Error::other(format!(
                "unexpected worktree admin dir for {id}"
            )));
        }
        std::fs::write(meta.join(&id), common.to_string_lossy().as_bytes())?;
        Ok(World { id, path })
    }

    /// The project's git store, from harness metadata, never from the world.
    fn common_dir(&self, world: &World) -> io::Result<PathBuf> {
        let text = std::fs::read_to_string(self.root.join("meta").join(&world.id))
            .map_err(|e| io::Error::other(format!("no metadata for world {}: {e}", world.id)))?;
        Ok(PathBuf::from(text.trim()))
    }

    /// git on the world's worktree, with paths from harness metadata.
    pub fn git(&self, world: &World, args: &[&str]) -> io::Result<String> {
        let admin = self.common_dir(world)?.join("worktrees").join(&world.id);
        let git_dir = format!("--git-dir={}", admin.display());
        let work_tree = format!("--work-tree={}", world.path.display());
        let mut all = vec![git_dir.as_str(), work_tree.as_str()];
        all.extend(args);
        git(&world.path, &all)
    }

    pub fn open(&self, id: &str) -> io::Result<World> {
        let path = self.root.join("worlds").join(id);
        // Ids are hex; anything else (`..` included) would name another dir.
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) || !path.is_dir() {
            return Err(io::Error::other(format!("no world {id}")));
        }
        Ok(World {
            id: id.to_string(),
            path,
        })
    }

    pub fn list(&self) -> io::Result<Vec<String>> {
        let Ok(entries) = std::fs::read_dir(self.root.join("worlds")) else {
            return Ok(Vec::new());
        };
        let mut ids: Vec<String> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        ids.sort();
        Ok(ids)
    }

    /// The command that runs `argv` inside the world's sandbox: read-only
    /// system, hidden home, private /tmp, no network, own pid namespace.
    pub fn sandbox_command(&self, world: &World, argv: &[String]) -> io::Result<Command> {
        if argv.is_empty() {
            return Err(io::Error::other("nothing to run"));
        }
        let common = self.common_dir(world)?;
        let mut cmd = Command::new("bwrap");
        // /run holds the session bus and daemon sockets: a path to the host
        // that no network namespace closes.
        cmd.args(["--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc"])
            .args(["--tmpfs", "/tmp", "--tmpfs", "/run", "--tmpfs"])
            .arg(&self.home);
        for p in self.ro_paths.iter().filter(|p| p.exists()) {
            cmd.arg("--ro-bind").arg(p).arg(p);
        }
        // Granted host paths: read-write, after the hidden home they sit in.
        for p in crate::grant::paths(self, &world.id)? {
            cmd.arg("--bind").arg(&p).arg(&p);
        }
        // Order matters: later mounts land on top of earlier ones. The
        // worktree's admin dir lives inside the store and stays read-only too:
        // its `commondir` file and config steer every harness git call.
        cmd.arg("--ro-bind")
            .arg(&common)
            .arg(&common)
            .arg("--bind")
            .arg(&world.path)
            .arg(&world.path)
            .args(["--unshare-all", "--die-with-parent", "--new-session"])
            // Every file in the world is the work: a bytecode cache written
            // by running the tests would fail a protected path, or land.
            // ponytail: Python only; another tool's untracked cache needs its
            // own switch, or the project's .gitignore.
            .args(["--setenv", "PYTHONDONTWRITEBYTECODE", "1"])
            .arg("--chdir")
            .arg(&world.path)
            .arg("--");
        // The cap is set inside the sandbox, so it binds the command and
        // everything it spawns. The data limit, not `-v`: since Linux 4.7 it
        // counts private writable memory only, so a runaway allocation stops
        // while address space merely reserved (Chromium needs over 64 GiB,
        // Go, V8) does not count.
        // ponytail: per process, not per tree; a cgroup if many processes add up.
        match self.memory_mb {
            Some(mb) => cmd
                .args(["sh", "-c", r#"ulimit -d "$0" && exec "$@""#])
                .arg((mb * 1024).to_string())
                .args(argv),
            None => cmd.args(argv),
        };
        Ok(cmd)
    }

    /// Everything that changed since the last checkpoint, untracked files included.
    pub fn diff(&self, world: &World) -> io::Result<String> {
        self.stage(world)?;
        self.git(world, &["diff", "--cached", "HEAD"])
    }

    /// Checkpoint the world. Returns the commit; a world with no change keeps its HEAD.
    pub fn commit(&self, world: &World, message: &str) -> io::Result<String> {
        self.stage(world)?;
        if !self.git(world, &["status", "--porcelain"])?.is_empty() {
            let mut args = COMMITTER.to_vec();
            args.extend(["commit", "-q", "-m", message]);
            self.git(world, &args)?;
        }
        self.git(world, &["rev-parse", "HEAD"])
    }

    /// Every file in the world is the work, but what a build writes is not:
    /// a project without a .gitignore would land `target/` or `node_modules/`.
    /// ponytail: a fixed list of tool outputs; a project that tracks one of
    /// these dirs on purpose cannot change it through fennec.
    fn stage(&self, world: &World) -> io::Result<String> {
        self.git(
            world,
            &[
                "add",
                "-A",
                "--",
                ".",
                ":(exclude,glob)**/target/**",
                ":(exclude,glob)**/node_modules/**",
                ":(exclude,glob)**/__pycache__/**",
                ":(exclude,glob)**/.venv/**",
            ],
        )
    }

    /// Back to the last checkpoint, exactly: ignored files go too.
    pub fn rollback(&self, world: &World) -> io::Result<()> {
        self.git(world, &["reset", "-q", "--hard", "HEAD"])?;
        self.git(world, &["clean", "-q", "-fdx"])?;
        Ok(())
    }

    /// Replays the world's checkpoints onto `onto`; on conflict the world is
    /// left as it was. Returns the new HEAD.
    pub fn rebase(&self, world: &World, onto: &str) -> io::Result<String> {
        let mut args = COMMITTER.to_vec();
        args.extend(["rebase", "-q", onto]);
        if let Err(e) = self.git(world, &args) {
            let _ = self.git(world, &["rebase", "--abort"]);
            return Err(e);
        }
        self.git(world, &["rev-parse", "HEAD"])
    }

    pub fn drop(&self, world: World) -> io::Result<()> {
        if !crate::grant::paths(self, &world.id)?.is_empty() {
            return Err(io::Error::other(
                "this world has a grant: keep or revert it before dropping the world",
            ));
        }
        let common = self.common_dir(&world)?;
        let git_dir = format!("--git-dir={}", common.display());
        // Delete the files ourselves rather than trust `worktree remove` with a
        // `.git` file the agent may have rewritten; prune then forgets the admin dir.
        // `remove_dir_all` does not follow symlinks.
        std::fs::remove_dir_all(&world.path)?;
        git(&self.root, &[&git_dir, "worktree", "prune"])?;
        git(
            &self.root,
            &[
                &git_dir,
                "branch",
                "-q",
                "-D",
                &format!("world/{}", world.id),
            ],
        )?;
        std::fs::remove_file(self.root.join("meta").join(&world.id))?;
        Ok(())
    }
}

/// Every harness git call: hooks off, whatever the repository says.
/// A project is its own repository: git must not walk up to a parent's (seen:
/// a project whose `.git` was deleted, inside a home that is a repository).
pub(crate) fn git(dir: &Path, args: &[&str]) -> io::Result<String> {
    let mut cmd = Command::new("git");
    if let Some(parent) = dir.parent().filter(|p| p.is_absolute()) {
        cmd.env("GIT_CEILING_DIRECTORIES", parent);
    }
    let out = cmd.arg("-C").arg(dir).args(NO_HOOKS).args(args).output()?;
    if !out.status.success() {
        return Err(io::Error::other(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Time-ordered ids: runs are named after their world, and "the latest run"
/// is the last id in lexical order. Wall time can step back (an NTP or WSL
/// host resync) and two forks can share a microsecond, so within a process
/// each id is strictly above the one before.
fn new_id() -> String {
    static LAST: AtomicU64 = AtomicU64::new(0);
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let now = t.as_secs() * 1_000_000 + u64::from(t.subsec_micros());
    let prev = LAST
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |p| Some(now.max(p + 1)))
        .unwrap_or_default();
    let id = now.max(prev + 1);
    format!("{:x}{:05x}", id / 1_000_000, id % 1_000_000)
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_only_grow_even_within_one_microsecond() {
        let ids: Vec<String> = (0..10_000).map(|_| super::new_id()).collect();
        assert!(
            ids.windows(2)
                .all(|w| w[0] < w[1] && w[0].len() == w[1].len())
        );
    }
}
