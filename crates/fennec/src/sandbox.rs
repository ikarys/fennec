//! fennec's sandbox world for commands (bwrap on Linux, Seatbelt on macOS): what a model runs sees the system read-only, its workdir writable, the human's secrets hidden and, by default, no network.
//! The file tools keep the in-process path guard; this only wraps commands.

use crate::domain::preferences::SandboxPrefs;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

// Each renderer is compiled everywhere so its pure tests run on any machine; only one is used.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod seatbelt;

#[derive(Debug, Clone, PartialEq)]
pub enum Sandbox {
    /// Commands run on the host (the human turned the sandbox off).
    Off,
    On {
        network: bool,
        hidden_dirs: Vec<PathBuf>,
        hidden_files: Vec<PathBuf>,
        writable: Vec<PathBuf>,
        read_only: Vec<PathBuf>,
        mounts: Vec<(PathBuf, PathBuf)>,
    },
}

impl Sandbox {
    /// The sandbox the human chose, hiding `data_dir` (fennec's store) and the secrets under `home`
    /// that exist on this machine (bwrap cannot mount over a missing path).
    pub fn from_prefs(prefs: &SandboxPrefs, home: Option<&Path>, data_dir: &Path) -> Sandbox {
        if !prefs.enabled {
            return Sandbox::Off;
        }

        let mut hidden_dirs = Vec::new();
        let mut hidden_files = Vec::new();
        let mut writable = Vec::new();

        if let Some(home) = home {
            for name in [
                ".ssh",
                ".gnupg",
                ".aws",
                ".azure",
                ".config/gcloud",
                ".config/gh",
                // the server settings, which hold the remote password's hash
                ".config/fennec",
                ".docker",
                ".kube",
                ".password-store",
            ] {
                let path = home.join(name);
                if path.is_dir() {
                    hidden_dirs.push(path);
                }
            }

            for name in [".netrc", ".git-credentials", ".npmrc", ".pypirc"] {
                let path = home.join(name);
                if path.is_file() {
                    hidden_files.push(path);
                }
            }

            // ponytail: a writable cache can be poisoned for the human's later runs; per-session
            // overlays (bwrap >= 0.10 `--overlay`) when that matters. Read-only, cargo cannot
            // download a crate and the agent falls back to committing `cargo vendor` copies.
            for name in [".npm", ".cache", ".cargo/registry", ".cargo/git"] {
                let path = home.join(name);
                if path.is_dir() {
                    writable.push(path);
                }
            }
        }

        if data_dir.is_dir() {
            hidden_dirs.push(data_dir.to_path_buf());
        }

        // A container daemon's socket is root on the host (`docker run -v /:/host`): unix sockets
        // ignore the read-only mount and the network namespace, so they are hidden outright.
        for socket in DAEMON_SOCKETS {
            if let Ok(path) = Path::new(socket).canonicalize()
                && !hidden_files.contains(&path)
            {
                hidden_files.push(path);
            }
        }

        Sandbox::On {
            network: prefs.network,
            hidden_dirs,
            hidden_files,
            writable,
            read_only: vec![],
            mounts: Vec::new(),
        }
    }

    /// The composition root's sandbox: the human's choice, hiding the
    /// directory of fennec's store (`db_path`) and the secrets under `$HOME`.
    pub fn for_store(prefs: &SandboxPrefs, db_path: &Path) -> Sandbox {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        // A store at the filesystem root would hide `/` itself: hide nothing then.
        let data_dir = db_path
            .canonicalize()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .filter(|dir| dir.parent().is_some())
            .unwrap_or_default();
        let mut sandbox = Sandbox::from_prefs(prefs, home.as_deref(), &data_dir);
        // The session runtime dir holds rootless docker/podman and D-Bus sockets.
        if let Sandbox::On { hidden_dirs, .. } = &mut sandbox
            && let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)
            && runtime.is_dir()
        {
            hidden_dirs.push(runtime);
        }
        sandbox
    }

    /// The same sandbox with `paths` mounted read-only inside the workdir.
    pub fn protecting(self, paths: Vec<PathBuf>) -> Sandbox {
        match self {
            Sandbox::Off => Sandbox::Off,
            Sandbox::On {
                network,
                hidden_dirs,
                hidden_files,
                writable,
                mounts,
                ..
            } => Sandbox::On {
                network,
                hidden_dirs,
                hidden_files,
                writable,
                read_only: paths,
                mounts,
            },
        }
    }

    /// The same sandbox with `mounts` bound read-only from source to destination.
    pub fn with_mounts(self, mounts: Vec<(PathBuf, PathBuf)>) -> Sandbox {
        match self {
            Sandbox::Off => Sandbox::Off,
            Sandbox::On {
                network,
                hidden_dirs,
                hidden_files,
                writable,
                read_only,
                ..
            } => Sandbox::On {
                network,
                hidden_dirs,
                hidden_files,
                writable,
                read_only,
                mounts,
            },
        }
    }

    /// The same sandbox with its network flag set (a plugin's network is its
    /// own choice, independent of the command sandbox's).
    pub fn with_network(mut self, network: bool) -> Sandbox {
        if let Sandbox::On {
            network: ref mut flag,
            ..
        } = self
        {
            *flag = network;
        }
        self
    }

    /// The same sandbox with `paths` writable too (bound after the hidden
    /// dirs, so a path inside fennec's hidden store is reachable).
    pub fn writing(mut self, paths: Vec<PathBuf>) -> Sandbox {
        if let Sandbox::On { writable, .. } = &mut self {
            writable.extend(paths);
        }
        self
    }

    /// The same sandbox with `workdir` itself read-only (later mounts win over
    /// its writable bind); `None` when the human turned the sandbox off, since
    /// nothing could then keep a command from writing.
    pub fn read_only(&self, workdir: &Path) -> Option<Sandbox> {
        let mut sandbox = self.clone();
        match &mut sandbox {
            Sandbox::Off => return None,
            Sandbox::On { read_only, .. } => read_only.push(workdir.to_path_buf()),
        }
        Some(sandbox)
    }

    /// `sh -c command` in `cwd`, wrapped in bwrap unless Off; `workdir` is the writable root.
    pub(super) fn command(&self, command: &str, workdir: &Path, cwd: &Path) -> Command {
        match self {
            Sandbox::Off => {
                let mut sh = Command::new("sh");
                sh.arg("-c").arg(command).current_dir(cwd);
                strip_secret_env(&mut sh);
                sh
            }
            Sandbox::On { .. } => {
                let mut wrapped = self.wrapper(workdir, cwd);
                wrapped.args(["sh", "-c", command]);
                // The sandbox passes fennec's environment through: the model would
                // read the endpoint key or a token with `env`.
                strip_secret_env(&mut wrapped);
                wrapped
            }
        }
    }

    /// `program args…` in `cwd`, wrapped in bwrap unless Off; `workdir` is the
    /// writable root. The MCP stdio client uses it to run a server in the same
    /// world as any command (secrets hidden, no network by default).
    pub fn spawn(&self, program: &str, args: &[String], workdir: &Path, cwd: &Path) -> Command {
        match self {
            Sandbox::Off => {
                let mut cmd = Command::new(program);
                cmd.args(args).current_dir(cwd);
                strip_secret_env(&mut cmd);
                cmd
            }
            Sandbox::On { .. } => {
                let mut wrapped = self.wrapper(workdir, cwd);
                wrapped.arg(program).args(args);
                strip_secret_env(&mut wrapped);
                wrapped
            }
        }
    }

    /// The sandbox program with this world's arguments, the command's tail still to add.
    #[cfg(not(target_os = "macos"))]
    fn wrapper(&self, workdir: &Path, cwd: &Path) -> Command {
        let mut bwrap = Command::new("bwrap");
        bwrap.args(bwrap_world_args(self, workdir, cwd)).arg("--");
        bwrap
    }

    /// The sandbox program with this world's arguments, the command's tail still to add.
    #[cfg(target_os = "macos")]
    fn wrapper(&self, workdir: &Path, cwd: &Path) -> Command {
        let mut sandbox_exec = Command::new(seatbelt::SANDBOX_EXEC);
        sandbox_exec
            .args(seatbelt::seatbelt_args(self, workdir))
            .env("TMPDIR", seatbelt::private_tmp(workdir))
            .current_dir(cwd);
        // bwrap sets these with --setenv; Seatbelt has no such flag.
        if let Sandbox::On { network: false, .. } = self {
            sandbox_exec
                .env("npm_config_offline", "true")
                .env("CARGO_NET_OFFLINE", "true");
        }
        sandbox_exec
    }
}

/// bwrap and a bare child both inherit fennec's environment: drop the secret
/// names so a command (or an MCP server) cannot read the endpoint key.
fn strip_secret_env(cmd: &mut Command) {
    for (name, _) in std::env::vars_os() {
        if is_secret_name(&name.to_string_lossy()) {
            cmd.env_remove(name);
        }
    }
}

/// Whether the sandbox (bwrap, Seatbelt) can build the world commands run in, on this machine (verified).
#[derive(Debug, Clone, PartialEq)]
pub enum SandboxHealth {
    Ready,
    Missing,
    Blocked(String),
}

/// Removes the temp dir the workdir's sandboxed commands wrote (only macOS makes one; elsewhere
/// there is nothing to remove). Call it before the workdir itself is removed.
pub fn remove_private_tmp(workdir: &Path) {
    let _ = std::fs::remove_dir_all(seatbelt::private_tmp_path(workdir));
}

/// Probes bwrap with the minimal world fennec needs, so a kernel setting that
/// forbids unprivileged user namespaces (Ubuntu 23.10+ AppArmor) surfaces in
/// the Settings page instead of every sandboxed command failing silently.
pub fn probe() -> SandboxHealth {
    if cfg!(target_os = "macos") {
        return health_from(seatbelt::probe());
    }
    let output = Command::new("bwrap")
        .args([
            "--ro-bind",
            "/",
            "/",
            "--dev",
            "/dev",
            "--proc",
            "/proc",
            "--unshare-all",
            "--die-with-parent",
            "--",
            "true",
        ])
        .output();
    health_from(output)
}

fn health_from(output: std::io::Result<std::process::Output>) -> SandboxHealth {
    match output {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => SandboxHealth::Missing,
        Err(error) => SandboxHealth::Blocked(error.to_string()),
        Ok(output) if output.status.success() => SandboxHealth::Ready,
        Ok(output) => SandboxHealth::Blocked(
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| {
                    format!(
                        "the sandbox exited with {}",
                        output.status.code().unwrap_or(-1)
                    )
                }),
        ),
    }
}

/// Container daemons' system sockets; `/var/run` is usually a link to `/run`.
const DAEMON_SOCKETS: [&str; 5] = [
    "/run/docker.sock",
    "/var/run/docker.sock",
    "/run/containerd/containerd.sock",
    "/run/podman/podman.sock",
    "/run/crio/crio.sock",
];

/// An environment variable that likely holds a credential.
// ponytail: a name heuristic; an allow-list of variables if one slips through.
fn is_secret_name(name: &str) -> bool {
    const MARKERS: [&str; 7] = [
        "TOKEN",
        "SECRET",
        "PASSWORD",
        "PASSWD",
        "API_KEY",
        "APIKEY",
        "CREDENTIAL",
    ];
    let upper = name.to_uppercase();
    MARKERS.iter().any(|marker| upper.contains(marker)) || upper.starts_with("AWS_")
}

/// The workdir's git metadata (`.git` directory in a checkout, `gitdir:` file
/// in a worktree); `None` when the workdir has no git metadata.
pub(super) fn git_metadata(workdir: &Path) -> Option<PathBuf> {
    let dot_git = workdir.join(".git");
    dot_git.symlink_metadata().ok().map(|_| dot_git)
}

/// The bwrap world, everything up to (and including) `--chdir cwd`; the
/// command tail after `--` is added by the caller, so a `sh -c` string and a
/// bare program+args (the MCP stdio client) share the same world.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn bwrap_world_args(sandbox: &Sandbox, workdir: &Path, cwd: &Path) -> Vec<OsString> {
    let Sandbox::On {
        network,
        hidden_dirs,
        hidden_files,
        writable,
        read_only,
        mounts,
    } = sandbox
    else {
        return Vec::new();
    };
    let network = *network;
    let mut args = vec![
        OsString::from("--ro-bind"),
        OsString::from("/"),
        OsString::from("/"),
        OsString::from("--dev"),
        OsString::from("/dev"),
        OsString::from("--proc"),
        OsString::from("/proc"),
        OsString::from("--tmpfs"),
        OsString::from("/tmp"),
    ];

    for dir in hidden_dirs {
        args.push(OsString::from("--tmpfs"));
        args.push(dir.as_os_str().to_os_string());
    }

    for file in hidden_files {
        args.push(OsString::from("--ro-bind"));
        args.push(OsString::from("/dev/null"));
        args.push(file.as_os_str().to_os_string());
    }

    for dir in writable {
        args.push(OsString::from("--bind"));
        args.push(dir.as_os_str().to_os_string());
        args.push(dir.as_os_str().to_os_string());
    }

    args.push(OsString::from("--bind"));
    args.push(workdir.as_os_str().to_os_string());
    args.push(workdir.as_os_str().to_os_string());

    // Dependency directories from the main checkout: read-only, because the
    // worktree lacks them and the sandbox has no network to install them.
    for (src, dst) in mounts {
        // The list is taken when the turn starts; starting a server since then
        // may have replaced the mount point with a link to the same copy, onto
        // which bwrap cannot bind.
        if dst.symlink_metadata().is_ok_and(|m| m.is_symlink()) {
            continue;
        }
        args.push(OsString::from("--ro-bind"));
        args.push(src.as_os_str().to_os_string());
        args.push(dst.as_os_str().to_os_string());
    }

    // Git's own metadata is read-only inside the sandbox: a model must not be
    // able to plant hooks or an fsmonitor that the host's git later runs.
    if let Some(dot_git) = git_metadata(workdir) {
        args.push(OsString::from("--ro-bind"));
        args.push(dot_git.as_os_str().to_os_string());
        args.push(dot_git.as_os_str().to_os_string());
    }

    // Later mounts win: protected paths stay read-only inside the writable
    // workdir.
    for path in read_only {
        args.push(OsString::from("--ro-bind"));
        args.push(path.as_os_str().to_os_string());
        args.push(path.as_os_str().to_os_string());
    }

    args.push(OsString::from("--unshare-all"));
    if network {
        args.push(OsString::from("--share-net"));
    }
    args.push(OsString::from("--die-with-parent"));
    args.push(OsString::from("--new-session"));

    if !network {
        args.push(OsString::from("--setenv"));
        args.push(OsString::from("npm_config_offline"));
        args.push(OsString::from("true"));
        args.push(OsString::from("--setenv"));
        args.push(OsString::from("CARGO_NET_OFFLINE"));
        args.push(OsString::from("true"));
    }

    args.push(OsString::from("--chdir"));
    args.push(cwd.as_os_str().to_os_string());
    args
}

#[cfg(test)]
fn bwrap_args(sandbox: &Sandbox, workdir: &Path, cwd: &Path, command: &str) -> Vec<OsString> {
    let mut args = bwrap_world_args(sandbox, workdir, cwd);
    args.push(OsString::from("--"));
    args.push(OsString::from("sh"));
    args.push(OsString::from("-c"));
    args.push(OsString::from(command));
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::test_support::temp_dir;
    use std::os::unix::process::ExitStatusExt;
    use std::path::Path;
    use std::process::ExitStatus;

    fn args_as_strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn bwrap_args_keep_the_mount_order_and_offline_env() {
        let args = bwrap_args(
            &Sandbox::On {
                network: false,
                hidden_dirs: vec![PathBuf::from("/home/user/.ssh")],
                hidden_files: vec![PathBuf::from("/home/user/.netrc")],
                writable: vec![PathBuf::from("/home/user/.npm")],
                read_only: vec![],
                mounts: vec![],
            },
            Path::new("/work"),
            Path::new("/cwd"),
            "echo hi",
        );

        assert_eq!(
            args_as_strings(&args),
            vec![
                "--ro-bind",
                "/",
                "/",
                "--dev",
                "/dev",
                "--proc",
                "/proc",
                "--tmpfs",
                "/tmp",
                "--tmpfs",
                "/home/user/.ssh",
                "--ro-bind",
                "/dev/null",
                "/home/user/.netrc",
                "--bind",
                "/home/user/.npm",
                "/home/user/.npm",
                "--bind",
                "/work",
                "/work",
                "--unshare-all",
                "--die-with-parent",
                "--new-session",
                "--setenv",
                "npm_config_offline",
                "true",
                "--setenv",
                "CARGO_NET_OFFLINE",
                "true",
                "--chdir",
                "/cwd",
                "--",
                "sh",
                "-c",
                "echo hi",
            ]
        );
    }

    #[test]
    fn bwrap_args_share_net_only_when_network_is_on() {
        let offline = bwrap_args(
            &Sandbox::On {
                network: false,
                hidden_dirs: vec![],
                hidden_files: vec![],
                writable: vec![],
                read_only: vec![],
                mounts: vec![],
            },
            Path::new("/work"),
            Path::new("/cwd"),
            "echo hi",
        );
        let offline = args_as_strings(&offline);
        assert!(!offline.contains(&"--share-net".to_string()));
        assert!(offline.contains(&"npm_config_offline".to_string()));
        assert!(offline.contains(&"CARGO_NET_OFFLINE".to_string()));

        let online = bwrap_args(
            &Sandbox::On {
                network: true,
                hidden_dirs: vec![],
                hidden_files: vec![],
                writable: vec![],
                read_only: vec![],
                mounts: vec![],
            },
            Path::new("/work"),
            Path::new("/cwd"),
            "echo hi",
        );
        let online = args_as_strings(&online);
        assert!(online.contains(&"--share-net".to_string()));
        assert!(!online.contains(&"npm_config_offline".to_string()));
        assert!(!online.contains(&"CARGO_NET_OFFLINE".to_string()));
    }

    #[test]
    fn bwrap_args_put_read_only_binds_after_the_workdir_bind() {
        let args = bwrap_args(
            &Sandbox::On {
                network: false,
                hidden_dirs: vec![],
                hidden_files: vec![],
                writable: vec![],
                read_only: vec![PathBuf::from("/work/protected")],
                mounts: vec![],
            },
            Path::new("/work"),
            Path::new("/cwd"),
            "echo hi",
        );

        assert_eq!(
            args_as_strings(&args),
            vec![
                "--ro-bind",
                "/",
                "/",
                "--dev",
                "/dev",
                "--proc",
                "/proc",
                "--tmpfs",
                "/tmp",
                "--bind",
                "/work",
                "/work",
                "--ro-bind",
                "/work/protected",
                "/work/protected",
                "--unshare-all",
                "--die-with-parent",
                "--new-session",
                "--setenv",
                "npm_config_offline",
                "true",
                "--setenv",
                "CARGO_NET_OFFLINE",
                "true",
                "--chdir",
                "/cwd",
                "--",
                "sh",
                "-c",
                "echo hi",
            ]
        );
    }

    #[test]
    fn bwrap_args_skip_a_mount_point_since_replaced_by_a_link() {
        let dir = std::env::temp_dir().join(format!("fennec-mount-link-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let link = dir.join("node_modules");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink("/main/web/node_modules", &link).unwrap();
        let args = bwrap_args(
            &Sandbox::On {
                network: false,
                hidden_dirs: vec![],
                hidden_files: vec![],
                writable: vec![],
                read_only: vec![],
                mounts: vec![(PathBuf::from("/main/web/node_modules"), link)],
            },
            &dir,
            &dir,
            "echo hi",
        );
        std::fs::remove_dir_all(&dir).unwrap();

        assert!(!args_as_strings(&args).contains(&"/main/web/node_modules".to_string()));
    }

    #[test]
    fn bwrap_args_put_dependency_mounts_after_the_workdir_bind() {
        let args = bwrap_args(
            &Sandbox::On {
                network: false,
                hidden_dirs: vec![],
                hidden_files: vec![],
                writable: vec![],
                read_only: vec![PathBuf::from("/work/protected")],
                mounts: vec![(
                    PathBuf::from("/main/web/node_modules"),
                    PathBuf::from("/work/web/node_modules"),
                )],
            },
            Path::new("/work"),
            Path::new("/cwd"),
            "echo hi",
        );

        assert_eq!(
            args_as_strings(&args),
            vec![
                "--ro-bind",
                "/",
                "/",
                "--dev",
                "/dev",
                "--proc",
                "/proc",
                "--tmpfs",
                "/tmp",
                "--bind",
                "/work",
                "/work",
                "--ro-bind",
                "/main/web/node_modules",
                "/work/web/node_modules",
                "--ro-bind",
                "/work/protected",
                "/work/protected",
                "--unshare-all",
                "--die-with-parent",
                "--new-session",
                "--setenv",
                "npm_config_offline",
                "true",
                "--setenv",
                "CARGO_NET_OFFLINE",
                "true",
                "--chdir",
                "/cwd",
                "--",
                "sh",
                "-c",
                "echo hi",
            ]
        );
    }

    #[test]
    fn bwrap_args_ro_bind_git_metadata_after_the_workdir_bind() {
        let workdir = temp_dir();
        std::fs::create_dir_all(workdir.join(".git")).unwrap();
        let args = bwrap_args(
            &Sandbox::On {
                network: false,
                hidden_dirs: vec![],
                hidden_files: vec![],
                writable: vec![],
                read_only: vec![],
                mounts: vec![],
            },
            &workdir,
            &workdir,
            "echo hi",
        );
        let args = args_as_strings(&args);
        let workdir_str = workdir.to_string_lossy().into_owned();
        let dot_git = format!("{workdir_str}/.git");

        let expected = [
            "--bind".to_string(),
            workdir_str.clone(),
            workdir_str,
            "--ro-bind".to_string(),
            dot_git.clone(),
            dot_git,
        ];
        assert!(args.windows(6).any(|window| window == expected), "{args:?}");

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn read_only_puts_the_workdir_read_only_bind_after_the_writable_bind() {
        let workdir = Path::new("/work");
        let sandbox = Sandbox::On {
            network: false,
            hidden_dirs: vec![],
            hidden_files: vec![],
            writable: vec![],
            read_only: vec![],
            mounts: vec![],
        }
        .read_only(workdir)
        .expect("a bwrap sandbox");
        let args = bwrap_args(&sandbox, workdir, Path::new("/cwd"), "echo hi");

        assert_eq!(
            args_as_strings(&args),
            vec![
                "--ro-bind",
                "/",
                "/",
                "--dev",
                "/dev",
                "--proc",
                "/proc",
                "--tmpfs",
                "/tmp",
                "--bind",
                "/work",
                "/work",
                "--ro-bind",
                "/work",
                "/work",
                "--unshare-all",
                "--die-with-parent",
                "--new-session",
                "--setenv",
                "npm_config_offline",
                "true",
                "--setenv",
                "CARGO_NET_OFFLINE",
                "true",
                "--chdir",
                "/cwd",
                "--",
                "sh",
                "-c",
                "echo hi",
            ]
        );

        assert_eq!(Sandbox::Off.read_only(workdir), None);
    }

    #[test]
    fn off_runs_sh_directly() {
        let workdir = Path::new("/work");
        let cwd = Path::new("/cwd");
        let command = Sandbox::Off.command("echo hi", workdir, cwd);

        assert_eq!(command.get_program().to_string_lossy(), "sh");
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args, vec!["-c", "echo hi"]);
        assert_eq!(command.get_current_dir(), Some(cwd));
    }

    #[test]
    fn from_prefs_lists_only_paths_that_exist() {
        let home = temp_dir();
        let data_dir = temp_dir();
        std::fs::create_dir_all(home.join(".ssh")).unwrap();
        std::fs::write(home.join(".netrc"), "token").unwrap();
        std::fs::create_dir_all(home.join(".cargo/registry")).unwrap();

        let prefs = SandboxPrefs {
            enabled: true,
            network: true,
        };
        let sandbox = Sandbox::from_prefs(&prefs, Some(&home), &data_dir);

        let Sandbox::On {
            network,
            hidden_dirs,
            hidden_files,
            writable,
            read_only,
            mounts,
        } = sandbox
        else {
            panic!("expected bwrap sandbox");
        };

        assert!(network);
        assert!(hidden_dirs.contains(&home.join(".ssh")));
        assert!(hidden_dirs.contains(&data_dir));
        assert!(!hidden_dirs.contains(&home.join(".aws")));
        assert!(hidden_files.contains(&home.join(".netrc")));
        assert!(!hidden_files.contains(&home.join(".npmrc")));
        assert_eq!(writable, vec![home.join(".cargo/registry")]);
        assert!(read_only.is_empty());
        assert!(mounts.is_empty());

        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn secret_names_are_recognised() {
        for name in [
            "FENNEC_API_KEY",
            "GITHUB_TOKEN",
            "AWS_PROFILE",
            "db_password",
        ] {
            assert!(is_secret_name(name), "{name}");
        }
        for name in ["PATH", "HOME", "FENNEC_MODEL", "LANG"] {
            assert!(!is_secret_name(name), "{name}");
        }
    }

    #[test]
    fn from_prefs_gives_off_when_disabled() {
        let home = temp_dir();
        let data_dir = temp_dir();
        std::fs::create_dir_all(home.join(".ssh")).unwrap();

        let prefs = SandboxPrefs {
            enabled: false,
            network: true,
        };
        assert_eq!(
            Sandbox::from_prefs(&prefs, Some(&home), &data_dir),
            Sandbox::Off
        );

        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::remove_dir_all(&data_dir);
    }

    fn bwrap_available() -> bool {
        Command::new("bwrap")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    #[test]
    fn e2e_writing_in_the_workdir_succeeds() {
        // End-to-end tests need a real bwrap; skip on machines without it.
        if !bwrap_available() {
            return;
        }

        let workdir = temp_dir();
        let sandbox = Sandbox::On {
            network: false,
            hidden_dirs: vec![],
            hidden_files: vec![],
            writable: vec![],
            read_only: vec![],
            mounts: vec![],
        };

        let output = sandbox
            .command("echo hello > probe.txt", &workdir, &workdir)
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        assert_eq!(
            std::fs::read_to_string(workdir.join("probe.txt")).unwrap(),
            "hello\n"
        );

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn e2e_container_daemon_sockets_are_hidden() {
        // Needs bwrap and a docker socket on the machine; skipped otherwise.
        if !bwrap_available() || !Path::new("/run/docker.sock").exists() {
            return;
        }

        let workdir = temp_dir();
        let data_dir = temp_dir();
        let prefs = SandboxPrefs {
            enabled: true,
            network: false,
        };
        let sandbox = Sandbox::from_prefs(&prefs, None, &data_dir);
        let output = sandbox
            .command(
                "test -S /run/docker.sock || test -S /var/run/docker.sock",
                &workdir,
                &workdir,
            )
            .output()
            .unwrap();
        assert!(!output.status.success(), "{:?}", output);

        let _ = std::fs::remove_dir_all(&workdir);
        let _ = std::fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn e2e_the_root_filesystem_is_read_only() {
        // End-to-end tests need a real bwrap; skip on machines without it.
        if !bwrap_available() {
            return;
        }

        let workdir = temp_dir();
        let sandbox = Sandbox::On {
            network: false,
            hidden_dirs: vec![],
            hidden_files: vec![],
            writable: vec![],
            read_only: vec![],
            mounts: vec![],
        };

        let output = sandbox
            .command("touch /etc/fennec-probe", &workdir, &workdir)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{:?}", output);

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn e2e_a_hidden_dir_is_not_readable() {
        // End-to-end tests need a real bwrap; skip on machines without it.
        if !bwrap_available() {
            return;
        }

        // /var/tmp is under the read-only root bind, unlike /tmp which bwrap replaces.
        if !Path::new("/var/tmp").is_dir() {
            return;
        }

        let hidden =
            Path::new("/var/tmp").join(format!("fennec-sandbox-hidden-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&hidden);
        if !hidden.is_dir() {
            return;
        }
        std::fs::write(hidden.join("secret"), "top secret").unwrap();

        let workdir = temp_dir();
        let sandbox = Sandbox::On {
            network: false,
            hidden_dirs: vec![hidden.clone()],
            hidden_files: vec![],
            writable: vec![],
            read_only: vec![],
            mounts: vec![],
        };

        let output = sandbox
            .command(
                &format!("cat {}", hidden.join("secret").display()),
                &workdir,
                &workdir,
            )
            .output()
            .unwrap();
        assert!(!output.status.success(), "{:?}", output);

        let _ = std::fs::remove_dir_all(&hidden);
        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn e2e_without_network_only_lo_exists() {
        // End-to-end tests need a real bwrap; skip on machines without it.
        if !bwrap_available() {
            return;
        }

        let workdir = temp_dir();
        let sandbox = Sandbox::On {
            network: false,
            hidden_dirs: vec![],
            hidden_files: vec![],
            writable: vec![],
            read_only: vec![],
            mounts: vec![],
        };

        let output = sandbox
            .command(
                // /sys is the host's sysfs under `--ro-bind /`; /proc/net
                // follows the sandbox's network namespace.
                "tail -n +3 /proc/net/dev | cut -d: -f1 | tr -d ' '",
                &workdir,
                &workdir,
            )
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "lo");

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn e2e_read_only_paths_are_read_only_in_the_workdir() {
        // End-to-end tests need a real bwrap; skip on machines without it.
        if !bwrap_available() {
            return;
        }

        let workdir = temp_dir();
        let protected = workdir.join("protected.txt");
        std::fs::write(&protected, "original").unwrap();

        let sandbox = Sandbox::On {
            network: false,
            hidden_dirs: vec![],
            hidden_files: vec![],
            writable: vec![],
            read_only: vec![protected.clone()],
            mounts: vec![],
        };

        let output = sandbox
            .command(
                &format!("echo changed > {}", protected.display()),
                &workdir,
                &workdir,
            )
            .output()
            .unwrap();
        assert!(!output.status.success(), "{:?}", output);
        assert_eq!(std::fs::read_to_string(&protected).unwrap(), "original");

        let sibling = workdir.join("sibling.txt");
        let output = sandbox
            .command(
                &format!("echo ok > {}", sibling.display()),
                &workdir,
                &workdir,
            )
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        assert_eq!(std::fs::read_to_string(&sibling).unwrap(), "ok\n");

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn e2e_git_metadata_is_read_only_in_the_workdir() {
        // End-to-end tests need a real bwrap; skip on machines without it.
        if !bwrap_available() {
            return;
        }

        let workdir = temp_dir();
        let dot_git = workdir.join(".git");
        std::fs::create_dir_all(&dot_git).unwrap();
        std::fs::write(dot_git.join("config"), "original").unwrap();

        let sandbox = Sandbox::On {
            network: false,
            hidden_dirs: vec![],
            hidden_files: vec![],
            writable: vec![],
            read_only: vec![],
            mounts: vec![],
        };

        let output = sandbox
            .command(
                "echo evil > .git/config; mkdir -p .git/hooks; echo x > .git/hooks/post-commit",
                &workdir,
                &workdir,
            )
            .output()
            .unwrap();
        assert!(!output.status.success(), "{:?}", output);
        assert_eq!(
            std::fs::read_to_string(dot_git.join("config")).unwrap(),
            "original"
        );
        assert!(!dot_git.join("hooks").join("post-commit").exists());

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn health_from_missing_when_bwrap_is_not_found() {
        let output = Err(std::io::Error::new(std::io::ErrorKind::NotFound, "bwrap"));
        assert_eq!(health_from(output), SandboxHealth::Missing);
    }

    #[test]
    fn health_from_ready_when_probe_succeeds() {
        let output = Ok(std::process::Output {
            status: ExitStatus::from_raw(0),
            stdout: vec![],
            stderr: vec![],
        });
        assert_eq!(health_from(output), SandboxHealth::Ready);
    }

    #[test]
    fn health_from_blocked_uses_the_first_stderr_line() {
        let output = Ok(std::process::Output {
            status: ExitStatus::from_raw(256),
            stdout: vec![],
            stderr: b"bwrap: setting up uid map: Permission denied\n".to_vec(),
        });
        assert_eq!(
            health_from(output),
            SandboxHealth::Blocked("bwrap: setting up uid map: Permission denied".to_string())
        );
    }

    #[test]
    fn health_from_blocked_reports_the_exit_status_when_stderr_is_empty() {
        let output = Ok(std::process::Output {
            status: ExitStatus::from_raw(256),
            stdout: vec![],
            stderr: vec![],
        });
        assert!(matches!(
            health_from(output),
            SandboxHealth::Blocked(detail) if detail.contains("exited")
        ));
    }
}
