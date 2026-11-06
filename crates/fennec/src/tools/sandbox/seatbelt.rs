//! The sandbox world on macOS, which has no bubblewrap: Seatbelt (`sandbox-exec`) renders the
//! same world as a profile, every path passed as a parameter so none can change the profile.

use super::Sandbox;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

pub(super) fn seatbelt_args(sandbox: &Sandbox, workdir: &Path) -> Vec<OsString> {
    match sandbox {
        Sandbox::Off => Vec::new(),
        Sandbox::On {
            network,
            hidden_dirs,
            hidden_files,
            writable,
            read_only,
            mounts: _,
        } => {
            let workdir = canonicalize(workdir);
            let hidden_dirs = hidden_dirs.iter().map(canonicalize).collect::<Vec<_>>();
            let hidden_files = hidden_files.iter().map(canonicalize).collect::<Vec<_>>();
            let writable = writable.iter().map(canonicalize).collect::<Vec<_>>();
            let read_only = read_only.iter().map(canonicalize).collect::<Vec<_>>();

            let profile =
                build_profile(*network, &hidden_dirs, &hidden_files, &writable, &read_only);

            let mut args = vec![OsString::from("-p"), profile.into()];
            args.push(OsString::from("-D"));
            args.push(param("WORKDIR", &workdir));
            args.push(OsString::from("-D"));
            args.push(param("TMPDIR", &canonicalize(private_tmp(&workdir))));

            for (i, path) in hidden_dirs.iter().enumerate() {
                args.push(OsString::from("-D"));
                args.push(param(&format!("HIDDEN_DIR_{i}"), path));
            }

            for (i, path) in hidden_files.iter().enumerate() {
                args.push(OsString::from("-D"));
                args.push(param(&format!("HIDDEN_FILE_{i}"), path));
            }

            for (i, path) in writable.iter().enumerate() {
                args.push(OsString::from("-D"));
                args.push(param(&format!("WRITABLE_{i}"), path));
            }

            for (i, path) in read_only.iter().enumerate() {
                args.push(OsString::from("-D"));
                args.push(param(&format!("READ_ONLY_{i}"), path));
            }

            args
        }
    }
}

pub(super) fn probe() -> std::io::Result<std::process::Output> {
    Command::new(SANDBOX_EXEC)
        .args(["-p", "(version 1)(allow default)", "/usr/bin/true"])
        .output()
}

/// The workdir's own temp dir, the only one its commands may write (`TMPDIR` points there): bwrap
/// gives each command a fresh /tmp, Seatbelt cannot, and the host's temp dir holds other files.
/// Kept across the workdir's commands, removed with its worktree (`remove_private_tmp`).
pub(super) fn private_tmp(workdir: &Path) -> PathBuf {
    let dir = private_tmp_path(workdir);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Where `private_tmp` lives, without creating it. The workdir must still exist: its canonical
/// path is the key.
pub(super) fn private_tmp_path(workdir: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    canonicalize(workdir).hash(&mut hasher);
    std::env::temp_dir()
        .join("fennec-sandbox")
        .join(format!("{:016x}", hasher.finish()))
}

fn canonicalize(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn param(name: &str, path: &Path) -> OsString {
    let mut value = OsString::from(name);
    value.push("=");
    value.push(path);
    value
}

fn build_profile(
    network: bool,
    hidden_dirs: &[PathBuf],
    hidden_files: &[PathBuf],
    writable: &[PathBuf],
    read_only: &[PathBuf],
) -> String {
    let mut profile = String::new();

    profile.push_str("(version 1)\n");
    profile.push_str("(allow default)\n");
    profile.push_str("(deny file-write*)\n");
    profile.push_str("(allow file-write*\n");
    profile.push_str(
        "  (literal \"/dev/null\") (literal \"/dev/zero\") (literal \"/dev/tty\") (literal \"/dev/dtracehelper\")\n",
    );
    profile.push_str("  (regex #\"^/dev/fd/\")\n");
    // Not /tmp: it is the host's, shared with every process. Commands get their own TMPDIR.
    profile.push_str("  (subpath (param \"TMPDIR\")))\n");
    profile.push_str("(allow file-write* (subpath (param \"WORKDIR\")))\n");

    for i in 0..hidden_dirs.len() {
        profile.push_str(&format!(
            r#"(deny file-read* file-write* (subpath (param "HIDDEN_DIR_{i}")))"#
        ));
        profile.push('\n');
    }

    for i in 0..hidden_files.len() {
        profile.push_str(&format!(
            r#"(deny file-read* file-write* (literal (param "HIDDEN_FILE_{i}")))"#
        ));
        profile.push('\n');
    }

    for i in 0..writable.len() {
        profile.push_str(&format!(
            r#"(allow file-read* file-write* (subpath (param "WRITABLE_{i}")))"#
        ));
        profile.push('\n');
    }

    // Seatbelt checks paths, so renaming a protected file's parent directory is not refused (bwrap's bind mount refuses it); the review gate still sees the protected file changed.
    for i in 0..read_only.len() {
        profile.push_str(&format!(
            r#"(deny file-write* (subpath (param "READ_ONLY_{i}")))"#
        ));
        profile.push('\n');
    }

    if network {
        for i in 0..hidden_files.len() {
            profile.push_str(&format!(
                r#"(deny network-outbound (remote unix-socket (path-literal (param "HIDDEN_FILE_{i}"))))"#
            ));
            profile.push('\n');
        }
    } else {
        profile.push_str("(deny network*)\n");
    }

    profile
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::test_support::temp_dir;

    fn profile_from_args(args: &[OsString]) -> Option<&str> {
        if args.first().is_some_and(|arg| arg == "-p") {
            args.get(1).and_then(|arg| arg.to_str())
        } else {
            None
        }
    }

    fn param_value(args: &[OsString], name: &str) -> Option<String> {
        args.windows(2)
            .filter(|pair| pair[0] == "-D")
            .filter_map(|pair| pair[1].to_str()?.split_once('='))
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.to_string())
    }

    #[test]
    fn off_gives_no_arguments() {
        let args = seatbelt_args(&Sandbox::Off, Path::new("/tmp/work"));
        assert!(args.is_empty());
    }

    #[test]
    fn offline_world_uses_params_not_paths() {
        let workdir = temp_dir();
        let hidden_dir = temp_dir();
        let hidden_file = hidden_dir.join("secret");
        let writable = temp_dir();
        let read_only = workdir.join("read-only");

        let sandbox = Sandbox::On {
            network: false,
            hidden_dirs: vec![hidden_dir.clone()],
            hidden_files: vec![hidden_file.clone()],
            writable: vec![writable.clone()],
            read_only: vec![read_only.clone()],
            mounts: vec![],
        };

        let args = seatbelt_args(&sandbox, &workdir);
        let profile = profile_from_args(&args).expect("profile argument");

        assert!(profile.contains("(deny network*)"));
        assert!(!args.is_empty());
        assert_eq!(args[0], OsString::from("-p"));
        assert_eq!(args.len() % 2, 0);

        for (flag, value) in args[2..].iter().step_by(2).zip(args[3..].iter().step_by(2)) {
            assert_eq!(flag, "-D");
            assert!(value.to_str().expect("param is UTF-8").contains('='));
        }

        for path in [&workdir, &hidden_dir, &hidden_file, &writable, &read_only] {
            let path = path.to_str().expect("path is UTF-8");
            assert!(!profile.contains(path), "profile spliced {path}");
        }
    }

    #[test]
    fn online_world_denies_hidden_file_unix_socket() {
        let workdir = temp_dir();
        let hidden_file = temp_dir().join("docker.sock");

        let sandbox = Sandbox::On {
            network: true,
            hidden_dirs: vec![],
            hidden_files: vec![hidden_file],
            writable: vec![],
            read_only: vec![],
            mounts: vec![],
        };

        let args = seatbelt_args(&sandbox, &workdir);
        let profile = profile_from_args(&args).expect("profile argument");

        assert!(!profile.contains("(deny network*)"));
        assert!(profile.contains(
            r#"(deny network-outbound (remote unix-socket (path-literal (param "HIDDEN_FILE_0"))))"#
        ));
        assert!(param_value(&args, "HIDDEN_FILE_0").is_some());
    }

    #[test]
    fn rule_order_is_workdir_hidden_writable_read_only() {
        let workdir = temp_dir();
        let hidden_dir = temp_dir();
        let hidden_file = hidden_dir.join("secret");
        let writable = temp_dir();
        let read_only = workdir.join("read-only");

        let sandbox = Sandbox::On {
            network: false,
            hidden_dirs: vec![hidden_dir],
            hidden_files: vec![hidden_file],
            writable: vec![writable],
            read_only: vec![read_only],
            mounts: vec![],
        };

        let args = seatbelt_args(&sandbox, &workdir);
        let profile = profile_from_args(&args).expect("profile argument");

        let workdir_pos = profile
            .find(r#"(allow file-write* (subpath (param "WORKDIR")))"#)
            .expect("workdir rule");
        let hidden_dir_pos = profile
            .find(r#"(deny file-read* file-write* (subpath (param "HIDDEN_DIR_0")))"#)
            .expect("hidden dir rule");
        let hidden_file_pos = profile
            .find(r#"(deny file-read* file-write* (literal (param "HIDDEN_FILE_0")))"#)
            .expect("hidden file rule");
        let writable_pos = profile
            .find(r#"(allow file-read* file-write* (subpath (param "WRITABLE_0")))"#)
            .expect("writable rule");
        let read_only_pos = profile
            .find(r#"(deny file-write* (subpath (param "READ_ONLY_0")))"#)
            .expect("read-only rule");

        assert!(workdir_pos < hidden_dir_pos);
        assert!(hidden_dir_pos < hidden_file_pos);
        assert!(hidden_file_pos < writable_pos);
        assert!(writable_pos < read_only_pos);
    }

    #[test]
    fn writable_inside_hidden_dir_appears_after_hidden() {
        let workdir = temp_dir();
        let hidden_dir = temp_dir();
        let writable = hidden_dir.join("scratch");

        let sandbox = Sandbox::On {
            network: true,
            hidden_dirs: vec![hidden_dir],
            hidden_files: vec![],
            writable: vec![writable],
            read_only: vec![],
            mounts: vec![],
        };

        let args = seatbelt_args(&sandbox, &workdir);
        let profile = profile_from_args(&args).expect("profile argument");

        let hidden_pos = profile
            .find(r#"(deny file-read* file-write* (subpath (param "HIDDEN_DIR_0")))"#)
            .expect("hidden dir rule");
        let writable_pos = profile
            .find(r#"(allow file-read* file-write* (subpath (param "WRITABLE_0")))"#)
            .expect("writable rule");

        assert!(hidden_pos < writable_pos);
    }

    #[test]
    fn remove_private_tmp_removes_the_workdirs_temp_dir() {
        let workdir = temp_dir();
        let tmp = private_tmp(&workdir);
        std::fs::write(tmp.join("left.o"), "build").unwrap();

        super::super::remove_private_tmp(&workdir);

        assert!(!tmp.exists());
    }

    #[test]
    fn canonicalizes_existing_paths_and_keeps_missing_paths() {
        let base = temp_dir();
        let existing = base
            .join("..")
            .join(base.file_name().expect("base has a file name"));

        let sandbox = Sandbox::On {
            network: true,
            hidden_dirs: vec![existing],
            hidden_files: vec![],
            writable: vec![],
            read_only: vec![],
            mounts: vec![],
        };

        let args = seatbelt_args(&sandbox, &base);
        let canonical = std::fs::canonicalize(&base).expect("base canonicalizes");
        assert_eq!(
            param_value(&args, "HIDDEN_DIR_0").expect("hidden dir param"),
            canonical.to_str().expect("canonical is UTF-8")
        );

        let missing = base.join("missing");
        let sandbox = Sandbox::On {
            network: true,
            hidden_dirs: vec![],
            hidden_files: vec![missing.clone()],
            writable: vec![],
            read_only: vec![],
            mounts: vec![],
        };

        let args = seatbelt_args(&sandbox, &base);
        assert_eq!(
            param_value(&args, "HIDDEN_FILE_0").expect("hidden file param"),
            missing.to_str().expect("missing is UTF-8")
        );
    }

    #[cfg(target_os = "macos")]
    mod macos {
        use super::*;
        use crate::tools::test_support::temp_dir;
        use std::fs;

        fn on(network: bool) -> Sandbox {
            Sandbox::On {
                network,
                hidden_dirs: Vec::new(),
                hidden_files: Vec::new(),
                writable: Vec::new(),
                read_only: Vec::new(),
                mounts: Vec::new(),
            }
        }

        fn run(
            sandbox: &Sandbox,
            workdir: &Path,
            script: &str,
        ) -> std::io::Result<std::process::Output> {
            Command::new(SANDBOX_EXEC)
                .args(seatbelt_args(sandbox, workdir))
                .args(["sh", "-c", script])
                .current_dir(workdir)
                .output()
        }

        #[test]
        fn write_in_workdir_succeeds() {
            let workdir = temp_dir();
            let sandbox = on(true);

            let out = run(&sandbox, &workdir, "touch work.txt").expect("sandbox-exec runs");
            assert!(out.status.success());
            assert!(workdir.join("work.txt").exists());
        }

        #[test]
        fn write_outside_workdir_fails() {
            let workdir = temp_dir();
            // The home directory: outside the workdir and the temp dirs, so nothing allows it.
            let home = PathBuf::from(std::env::var_os("HOME").expect("HOME is set"));
            let outside = home.join(format!(".fennec-seatbelt-test-{}", std::process::id()));

            let script = format!("touch {}", outside.to_str().unwrap());
            let out = run(&on(false), &workdir, &script).expect("sandbox-exec runs");
            let created = outside.exists();
            let _ = fs::remove_file(&outside);
            assert!(!out.status.success());
            assert!(!created);
        }

        #[test]
        fn hidden_dir_read_fails_writable_inside_succeeds() {
            let workdir = temp_dir();
            let hidden = temp_dir();
            let writable = hidden.join("writable");
            fs::create_dir_all(&writable).expect("create writable dir");

            let hidden_file = hidden.join("secret.txt");
            fs::write(&hidden_file, b"secret").expect("write hidden file");

            let sandbox = Sandbox::On {
                network: true,
                hidden_dirs: vec![hidden.clone()],
                hidden_files: vec![],
                writable: vec![writable.clone()],
                read_only: vec![],
                mounts: vec![],
            };

            let read_script = format!("cat {}", hidden_file.to_str().unwrap());
            let read_out = run(&sandbox, &workdir, &read_script).expect("sandbox-exec runs");
            assert!(!read_out.status.success());

            let write_script = format!("touch {}", writable.join("ok.txt").to_str().unwrap());
            let write_out = run(&sandbox, &workdir, &write_script).expect("sandbox-exec runs");
            assert!(write_out.status.success());
        }

        #[test]
        fn read_only_file_write_fails_sibling_writable() {
            let workdir = temp_dir();
            let protected = workdir.join("protected.txt");
            fs::write(&protected, b"read-only").expect("write protected file");

            let sandbox = Sandbox::On {
                network: true,
                hidden_dirs: vec![],
                hidden_files: vec![],
                writable: vec![],
                read_only: vec![protected],
                mounts: vec![],
            };

            let protected_out =
                run(&sandbox, &workdir, "echo x > protected.txt").expect("sandbox-exec runs");
            assert!(!protected_out.status.success());

            let sibling_out =
                run(&sandbox, &workdir, "touch sibling.txt").expect("sandbox-exec runs");
            assert!(sibling_out.status.success());
        }

        #[test]
        fn offline_network_fails() {
            let workdir = temp_dir();
            let sandbox = on(false);

            let out = run(&sandbox, &workdir, "/usr/bin/nc -z -G 2 1.1.1.1 443")
                .expect("sandbox-exec runs");
            assert!(!out.status.success());
        }

        #[test]
        fn probe_succeeds() {
            let out = probe().expect("probe runs");
            assert!(out.status.success());
        }
    }
}
