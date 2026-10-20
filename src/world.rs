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
//!
//! Where a project's worlds go is the human's choice, in the project's own
//! git config (`fennec.workspace`): unset or `here`, the project folder
//! itself, so the work happens where they look; `apart`, a folder next to the
//! project (`<name>-fennec/`); or a directory of their choosing. A world in
//! the folder is its branch checked out there: the folder must be clean to
//! start, holds one world at a time, and goes back to its branch when the
//! world is dropped or lands.

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
    /// New worlds of a project go apart whatever it chose: work no one
    /// watches (fennec-lead's Fleet) must not hold the folder while it
    /// waits for the human.
    pub apart: bool,
    /// New worlds of a project start from this branch when it exists,
    /// rather than from the folder's HEAD: work queued on a branch of its
    /// own builds on what landed there before.
    pub base: Option<String>,
}

pub struct World {
    pub id: String,
    pub path: PathBuf,
}

/// The crates a Cargo project's `path = "..."` dependencies point to, outside it.
/// The Cargo.toml is the agent's to write: only a folder holding a crate is
/// taken, and [`Worlds::unopenable`] filters the rest where it is mounted.
/// ponytail: the top Cargo.toml only, resolved from `dir`; a world kept apart
/// from the project folder does not find `../x`, read the project's own then.
fn path_deps(dir: &Path) -> Vec<PathBuf> {
    let Ok(dir) = std::fs::canonicalize(dir) else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(dir.join("Cargo.toml")) else {
        return Vec::new();
    };
    text.split("path")
        .skip(1)
        .filter_map(|rest| {
            let rest = rest.trim_start().strip_prefix('=')?.trim_start();
            let rest = rest.strip_prefix('"')?;
            std::fs::canonicalize(dir.join(&rest[..rest.find('"')?])).ok()
        })
        .filter(|p| p.join("Cargo.toml").is_file() && !p.starts_with(&dir))
        .collect()
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
            // Toolchains the human installed in their home: Rust, uv and the
            // Pythons it manages, Node through nvm, the tools pipx links into
            // ~/.local/bin. Read-only: a project's
            // packages go in the project (`.venv`, `node_modules`), never here.
            None => [
                ".cargo",
                ".rustup",
                ".local/bin",
                ".local/share/pipx",
                ".local/share/uv/python",
                ".nvm",
                ".local/share/nvm",
            ]
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
            apart: false,
            base: None,
        })
    }

    /// Fork a project (any git directory) or an existing world, from its last commit.
    pub fn fork(&self, source: &str) -> io::Result<World> {
        let (common, rev, place) = match self.open(source) {
            Ok(world) => {
                let (common, _, home) = self.meta(&world.id)?;
                let place = match home {
                    // Dropped, the child gives the folder back to its parent.
                    Some(home) => {
                        Place::Here(world.path.clone(), home, format!("world/{}", world.id))
                    }
                    None => Place::Apart(self.root.join("worlds")),
                };
                (common, format!("world/{}", world.id), place)
            }
            Err(_) => {
                let project = Path::new(source);
                let common = git(
                    project,
                    &["rev-parse", "--path-format=absolute", "--git-common-dir"],
                )?;
                let place = if self.apart {
                    apart(project)?
                } else {
                    place(project)?
                };
                let base = self.base.as_deref().and_then(|b| {
                    git(
                        project,
                        &["rev-parse", "-q", "--verify", &format!("refs/heads/{b}")],
                    )
                    .ok()
                });
                let rev = match base {
                    Some(rev) => rev,
                    None => git(project, &["rev-parse", "HEAD"])?,
                };
                (PathBuf::from(common), rev, place)
            }
        };
        ignore_cache(&common)?;
        let id = new_id();
        let meta = self.root.join("meta");
        std::fs::create_dir_all(&meta)?;
        let dir = match place {
            Place::Here(folder, home, back) => {
                // The folder's own work tree: the branch is checked out where the
                // human looks. From a world, local changes (their edits) carry over.
                let branch = format!("world/{id}");
                git(&folder, &["checkout", "-q", "-b", &branch, &rev])?;
                let record = format!("{}\n{}\n{home}\n{back}", common.display(), folder.display());
                std::fs::write(meta.join(&id), record)?;
                return Ok(World { id, path: folder });
            }
            Place::Apart(dir) => dir,
        };
        let path = dir.join(&id);
        std::fs::create_dir_all(&dir)?;
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
        std::fs::write(
            meta.join(&id),
            format!("{}\n{}", common.display(), path.display()),
        )?;
        Ok(World { id, path })
    }

    /// From harness metadata, never from the world: the project's git store,
    /// the world's folder (absent in old records: under the store), and for
    /// a world in the project folder, the branch that folder came from.
    fn meta(&self, id: &str) -> io::Result<(PathBuf, Option<PathBuf>, Option<String>)> {
        Ok(self.meta_all(id)?.0)
    }

    /// `meta`, and for a world in the project folder, the branch the folder
    /// goes back to when the world is dropped: the project's, or the world
    /// it was forked from.
    #[allow(clippy::type_complexity)]
    fn meta_all(
        &self,
        id: &str,
    ) -> io::Result<((PathBuf, Option<PathBuf>, Option<String>), Option<String>)> {
        let text = std::fs::read_to_string(self.root.join("meta").join(id))
            .map_err(|e| io::Error::other(format!("no metadata for world {id}: {e}")))?;
        let mut lines = text.lines();
        let common = PathBuf::from(lines.next().unwrap_or("").trim());
        let path = lines.next().map(PathBuf::from);
        let home = lines.next().map(str::to_string);
        let back = lines.next().map(str::to_string).or(home.clone());
        Ok(((common, path, home), back))
    }

    fn common_dir(&self, world: &World) -> io::Result<PathBuf> {
        Ok(self.meta(&world.id)?.0)
    }

    /// The branch a world in the project folder came from; `None` for a
    /// world apart.
    pub fn home(&self, world: &World) -> io::Result<Option<String>> {
        Ok(self.meta(&world.id)?.2)
    }

    /// The project a world was forked from: its git store's folder, from
    /// harness metadata.
    pub fn project(&self, world: &World) -> io::Result<PathBuf> {
        let common = self.common_dir(world)?;
        Ok(common.parent().map_or(common.clone(), Path::to_path_buf))
    }

    /// A world in the project folder is there only while its branch is
    /// checked out: anything else in the folder is not this world.
    fn present(&self, world: &World) -> io::Result<()> {
        if self.home(world)?.is_some()
            && git(&world.path, &["branch", "--show-current"])? != format!("world/{}", world.id)
        {
            return Err(io::Error::other(format!(
                "the project folder no longer holds world {}",
                world.id
            )));
        }
        Ok(())
    }

    /// git on the world's worktree, with paths from harness metadata.
    pub fn git(&self, world: &World, args: &[&str]) -> io::Result<String> {
        let common = self.common_dir(world)?;
        // A world in the project folder is the store's main work tree, and
        // only while its branch is checked out there: never the human's.
        let admin = match self.home(world)? {
            Some(_) => {
                self.present(world)?;
                common
            }
            None => common.join("worktrees").join(&world.id),
        };
        let git_dir = format!("--git-dir={}", admin.display());
        let work_tree = format!("--work-tree={}", world.path.display());
        let mut all = vec![git_dir.as_str(), work_tree.as_str()];
        all.extend(args);
        git(&world.path, &all)
    }

    pub fn open(&self, id: &str) -> io::Result<World> {
        // Ids are hex; anything else (`..` included) would name another file.
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(io::Error::other(format!("no world {id}")));
        }
        let path = match self.meta(id) {
            Ok((_, Some(path), _)) => path,
            _ => self.root.join("worlds").join(id),
        };
        if !path.is_dir() {
            return Err(io::Error::other(format!("no world {id}")));
        }
        Ok(World {
            id: id.to_string(),
            path,
        })
    }

    pub fn list(&self) -> io::Result<Vec<String>> {
        let Ok(entries) = std::fs::read_dir(self.root.join("meta")) else {
            return Ok(Vec::new());
        };
        let mut ids: Vec<String> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        ids.sort();
        Ok(ids)
    }

    /// What a world holds, for a person deciding whether to throw it away:
    /// its project, whether it sits in that folder (and is checked out
    /// there), when it was made, its size apart, and the subjects of the
    /// commits no other branch has.
    pub fn facts(&self, id: &str) -> io::Result<serde_json::Value> {
        let ((common, path, home), _) = self.meta_all(id)?;
        let project = common.parent().map(Path::to_path_buf).unwrap_or_default();
        let branch = format!("refs/heads/world/{id}");
        let git_dir = format!("--git-dir={}", common.display());
        // --branches matches its patterns without the refs/heads/ prefix.
        let exclude = format!("--exclude=world/{id}");
        let only = git(
            &self.root,
            &[
                &git_dir,
                "log",
                "--format=%s",
                &branch,
                "--not",
                &exclude,
                "--branches",
            ],
        )
        .unwrap_or_default();
        let checked_out = home.is_some()
            && git(&project, &["branch", "--show-current"]).ok() == Some(format!("world/{id}"));
        let dir = path.unwrap_or_else(|| self.root.join("worlds").join(id));
        // Ids are hex seconds then five hex digits of microseconds (`new_id`).
        let since = id
            .get(..id.len().saturating_sub(5))
            .and_then(|s| u64::from_str_radix(s, 16).ok());
        Ok(serde_json::json!({
            "id": id,
            "project": project,
            "in_folder": home.is_some(),
            "checked_out": checked_out,
            "since": since,
            "bytes": if home.is_none() { size(&dir) } else { 0 },
            "commits": only.lines().collect::<Vec<_>>(),
        }))
    }

    /// Removes a world whatever is left of it. A start or a drop cut short
    /// leaves part of its branch, folder and metadata, which `drop` needs whole.
    pub fn forget(&self, id: &str) -> io::Result<()> {
        if let Ok(world) = self.open(id)
            && self.drop(world).is_ok()
        {
            return Ok(());
        }
        if !crate::grant::paths(self, id)?.is_empty() {
            return Err(io::Error::other(
                "this world has a grant: keep or revert it before dropping the world",
            ));
        }
        let ((common, path, home), _) = self.meta_all(id)?;
        let git_dir = format!("--git-dir={}", common.display());
        if home.is_none() {
            let dir = path.unwrap_or_else(|| self.root.join("worlds").join(id));
            if dir.is_dir() {
                std::fs::remove_dir_all(&dir)?;
            }
            git(&self.root, &[&git_dir, "worktree", "prune"])?;
        }
        let branch = format!("refs/heads/world/{id}");
        if git(
            &self.root,
            &[&git_dir, "rev-parse", "-q", "--verify", &branch],
        )
        .is_ok()
        {
            // Checked out in the project folder, git refuses: the folder stays as it is.
            git(
                &self.root,
                &[&git_dir, "branch", "-q", "-D", &format!("world/{id}")],
            )?;
        }
        std::fs::remove_file(self.root.join("meta").join(id))?;
        let _ = std::fs::remove_file(self.root.join("skills").join(id));
        Ok(())
    }

    /// The command that runs `argv` inside the world's sandbox: read-only
    /// system, hidden home, private /tmp, no network, own pid namespace.
    pub fn sandbox_command(&self, world: &World, argv: &[String]) -> io::Result<Command> {
        self.sandboxed(world, argv, false)
    }

    /// The sandbox an agent works in: the same, on the network, so it
    /// installs what the project needs (asked by the human, 2026-10-20: a
    /// project with dependencies could never be built nor proven). What
    /// judges the work runs without it ([`Self::sandbox_command`]).
    pub fn sandbox_work(&self, world: &World, argv: &[String]) -> io::Result<Command> {
        self.sandboxed(world, argv, true)
    }

    /// The same sandbox on the host's network: only for the product the
    /// human asked to open in their own browser (`proof::page`).
    pub fn sandbox_on_host_network(&self, world: &World, argv: &[String]) -> io::Result<Command> {
        self.sandboxed(world, argv, true)
    }

    fn sandboxed(&self, world: &World, argv: &[String], net: bool) -> io::Result<Command> {
        let common = self.common_dir(world)?;
        // Granted host paths: read-write, after the hidden home they sit in.
        let mut mounts: Vec<(bool, PathBuf, PathBuf)> = crate::grant::paths(self, &world.id)?
            .into_iter()
            .map(|p| (true, p.clone(), p))
            .collect();
        mounts.extend(self.skill_mounts(&crate::skill::mounted(self, &world.id)));
        // Order matters: later mounts land on top of earlier ones. The store
        // comes last: a world in the project folder holds it (`.git`), and
        // the worktree's admin dir lives inside it; read-only either way, as
        // its config steers every harness git call.
        mounts.push((true, world.path.clone(), world.path.clone()));
        // Caches are the project's, shared by all its worlds: a world apart
        // would otherwise fetch and build everything again, and each would
        // take as much disk (asked by the human, 2026-10-20). A Cargo build
        // folder goes over the world's own `target/`, so paths stay as the
        // project writes them.
        let project = common.parent().unwrap_or(&common).to_path_buf();
        let cache = project.join(CACHE);
        if world.path != project {
            std::fs::create_dir_all(&cache)?;
            mounts.push((true, cache.clone(), cache.clone()));
            if world.path.join("Cargo.toml").is_file() {
                let target = cache.join("target");
                std::fs::create_dir_all(&target)?;
                mounts.push((true, target, world.path.join("target")));
            }
        }
        mounts.push((false, common.clone(), common));
        self.jail(&world.path, &cache, &mounts, net, argv)
    }

    /// The command that runs `argv` in a sandbox over a folder of the
    /// human's that is no world: the same isolation, `dir` and `grants` —
    /// host folders the human opened for the chat — read-write when `rw`,
    /// and `skills` read-only. Only for a folder they chose (`chat`).
    pub fn sandbox_folder(
        &self,
        dir: &Path,
        skills: &[(String, PathBuf)],
        grants: &[PathBuf],
        rw: bool,
        argv: &[String],
    ) -> io::Result<Command> {
        let mut mounts = self.skill_mounts(skills);
        for g in grants {
            mounts.push((rw, g.clone(), g.clone()));
        }
        mounts.push((rw, dir.to_path_buf(), dir.to_path_buf()));
        // On the network, as an agent's work is (`sandbox_work`).
        self.jail(dir, &dir.join(CACHE), &mounts, true, argv)
    }

    /// Why `dir` cannot be opened in a sandbox, as a chat's folder, a folder
    /// opened for it or a path dependency: it must sit strictly inside the
    /// home and never over the store.
    pub(crate) fn unopenable(&self, dir: &Path) -> Option<String> {
        let home = std::fs::canonicalize(&self.home).unwrap_or(self.home.clone());
        let store = std::fs::canonicalize(&self.root).unwrap_or(self.root.clone());
        if !dir.starts_with(&home) || dir == home {
            return Some(format!(
                "{} is not a folder strictly inside {}",
                dir.display(),
                home.display()
            ));
        }
        if dir.starts_with(&store) || store.starts_with(dir) {
            return Some(format!("{} overlaps the world store", dir.display()));
        }
        None
    }

    /// The [`TOOLS`] a sandboxed command finds on its PATH: in a folder the
    /// home hides, only under the toolchains re-exposed ([`Self::ro_paths`]).
    /// Looked up once: a PATH through slow mounts (WSL's `/mnt/c`) takes a
    /// second to scan.
    /// ponytail: the first `Worlds` decides for the process; a server whose
    /// toolchains change needs a restart.
    pub fn tools(&self) -> &'static [&'static str] {
        static FOUND: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
        FOUND.get_or_init(|| {
            let path = std::env::var_os("PATH").unwrap_or_default();
            let dirs: Vec<PathBuf> = std::env::split_paths(&path)
                .filter(|d| {
                    !d.starts_with(&self.home) || self.ro_paths.iter().any(|r| d.starts_with(r))
                })
                .collect();
            TOOLS
                .into_iter()
                .filter(|t| dirs.iter().any(|d| d.join(t).is_file()))
                .collect()
        })
    }

    /// Skills, read-only, where the brief says they are.
    fn skill_mounts(&self, skills: &[(String, PathBuf)]) -> Vec<(bool, PathBuf, PathBuf)> {
        skills
            .iter()
            .filter(|(name, dir)| !name.contains('/') && dir.is_dir())
            .map(|(name, dir)| (false, dir.clone(), self.home.join(".skills").join(name)))
            .collect()
    }

    /// bwrap over `mounts` (read-write?, host path, sandbox path), in order,
    /// package managers caching in `cache`.
    fn jail(
        &self,
        dir: &Path,
        cache: &Path,
        mounts: &[(bool, PathBuf, PathBuf)],
        net: bool,
        argv: &[String],
    ) -> io::Result<Command> {
        if argv.is_empty() {
            return Err(io::Error::other("nothing to run"));
        }
        let mut cmd = Command::new("bwrap");
        // /run holds the session bus and daemon sockets: a path to the host
        // that no network namespace closes.
        cmd.args(["--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc"])
            .args(["--tmpfs", "/tmp", "--tmpfs", "/run", "--tmpfs"])
            .arg(&self.home);
        for p in self.ro_paths.iter().filter(|p| p.exists()) {
            cmd.arg("--ro-bind").arg(p).arg(p);
        }
        // A Cargo path dependency in the hidden home: without it the project
        // cannot build, so QA could not even start it.
        // Never the home, the store or anything around them: `path = "~/.ssh"`
        // would show what the hidden home hides.
        for p in path_deps(dir)
            .into_iter()
            .filter(|p| self.unopenable(p).is_none())
        {
            cmd.arg("--ro-bind").arg(&p).arg(&p);
        }
        for (rw, from, to) in mounts {
            cmd.arg(if *rw { "--bind" } else { "--ro-bind" })
                .arg(from)
                .arg(to);
        }
        cmd.args(["--unshare-all", "--die-with-parent", "--new-session"])
            .args(if net { &["--share-net"][..] } else { &[] })
            // Every file in the world is the work: a bytecode cache written
            // by running the tests would fail a protected path, or land.
            // ponytail: Python only; another tool's untracked cache needs its
            // own switch, or the project's .gitignore.
            .args(["--setenv", "PYTHONDONTWRITEBYTECODE", "1"])
            // No Python downloaded behind the human's back: the project uses
            // one the machine has.
            .args(["--setenv", "UV_PYTHON_DOWNLOADS", "never"]);
        // Package managers download into the home, hidden and emptied after
        // each command: every command would fetch again, and the checks,
        // without network, would find nothing. Their caches go in the
        // project's `.fennec/` instead, never committed (`BUILD_OUTPUT`), gone
        // with the project: nothing piles up elsewhere (asked by the human,
        // 2026-10-20).
        for (var, sub) in CACHES {
            cmd.arg("--setenv").arg(var).arg(cache.join(sub));
        }
        let mut maven = std::ffi::OsString::from("-Dmaven.repo.local=");
        maven.push(cache.join("m2"));
        cmd.arg("--setenv").arg("MAVEN_OPTS").arg(maven);
        if !net {
            for (var, value) in OFFLINE {
                cmd.args(["--setenv", var, value]);
            }
        }
        cmd
            // `~` is the hidden home, where skills are mounted.
            .arg("--setenv")
            .arg("HOME")
            .arg(&self.home)
            .arg("--chdir")
            .arg(dir)
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
        if !self.status(world)?.is_empty() {
            let mut args = COMMITTER.to_vec();
            args.extend(["commit", "-q", "-m", message]);
            self.git(world, &args)?;
        }
        self.git(world, &["rev-parse", "HEAD"])
    }

    fn stage(&self, world: &World) -> io::Result<String> {
        let mut args = vec!["add", "-A", "--", "."];
        args.extend(BUILD_OUTPUT);
        self.git(world, &args)
    }

    /// What changed in the work since the last checkpoint, build output left
    /// out as [`Self::commit`] leaves it: a `.pyc` a build wrote is no change.
    pub fn status(&self, world: &World) -> io::Result<String> {
        let mut args = vec!["status", "--porcelain", "--", "."];
        args.extend(BUILD_OUTPUT);
        self.git(world, &args)
    }

    /// Back to the last checkpoint: what the work changed goes, ignored files
    /// stay (`node_modules`, `target`): a proof's try is undone before the
    /// product starts, without network to install them again. In the
    /// project folder, what goes is set aside instead.
    pub fn rollback(&self, world: &World) -> io::Result<()> {
        if self.home(world)?.is_some() {
            // The human may have edited their folder while fennec worked: what
            // goes is set aside in the stash, never lost.
            let mut args = COMMITTER.to_vec();
            args.extend(["stash", "push", "-q", "-u", "-m", "fennec: try undone"]);
            self.git(world, &args)?;
            return Ok(());
        }
        self.git(world, &["reset", "-q", "--hard", "HEAD"])?;
        // The package caches stay: an undone try did not make them wrong.
        self.git(world, &["clean", "-q", "-fd", "-e", "/.fennec"])?;
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

    /// A world in the project folder set aside: its work checkpointed and
    /// kept on its branch, the folder back on the human's branch, so other
    /// work can start there. [`Self::unpark`] checks it out again. Nothing to
    /// do for a world apart, or one the folder already left.
    pub fn park(&self, world: &World) -> io::Result<()> {
        let (_, _, home) = self.meta(&world.id)?;
        let Some(home) = home else {
            return Ok(());
        };
        if git(&world.path, &["branch", "--show-current"])? != format!("world/{}", world.id) {
            return Ok(());
        }
        self.commit(world, "fennec: set aside")?;
        git(&world.path, &["checkout", "-q", &home])?;
        Ok(())
    }

    /// A parked world back in the project folder, which must be free: on the
    /// human's branch, with nothing uncommitted.
    pub fn unpark(&self, world: &World) -> io::Result<()> {
        let (_, _, home) = self.meta(&world.id)?;
        let branch = format!("world/{}", world.id);
        let now = git(&world.path, &["branch", "--show-current"])?;
        if home.is_none() || now == branch {
            return Ok(());
        }
        if now.starts_with("world/") {
            return Err(io::Error::other(
                "another task works in the folder: wait until it finishes",
            ));
        }
        if !git(&world.path, &["status", "--porcelain"])?.is_empty() {
            return Err(io::Error::other("the folder has changes not committed yet"));
        }
        git(&world.path, &["checkout", "-q", &branch])?;
        Ok(())
    }

    pub fn drop(&self, world: World) -> io::Result<()> {
        if !crate::grant::paths(self, &world.id)?.is_empty() {
            return Err(io::Error::other(
                "this world has a grant: keep or revert it before dropping the world",
            ));
        }
        let ((common, _, home), back) = self.meta_all(&world.id)?;
        let git_dir = format!("--git-dir={}", common.display());
        let branch = format!("world/{}", world.id);
        if let (Some(home), Some(back)) = (home, back) {
            // The world it was forked from may be gone: a correction that won
            // replaced it, so the folder goes back to the human's branch.
            let back = if git(
                &world.path,
                &["rev-parse", "-q", "--verify", &format!("refs/heads/{back}")],
            )
            .is_ok()
            {
                back
            } else {
                home
            };
            // The folder goes back to its branch, unless it already left this
            // world (landed, or another world of the same request took over).
            if git(&world.path, &["branch", "--show-current"])? == branch {
                self.rollback(&world)?;
                git(&world.path, &["checkout", "-q", &back])?;
            }
            git(&world.path, &["branch", "-q", "-D", &branch])?;
            std::fs::remove_file(self.root.join("meta").join(&world.id))?;
            let _ = std::fs::remove_file(self.root.join("skills").join(&world.id));
            return Ok(());
        }
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
        let _ = std::fs::remove_file(self.root.join("skills").join(&world.id));
        Ok(())
    }
}

enum Place {
    /// The project folder, the branch it is on, and the one to go back to.
    Here(PathBuf, String, String),
    /// A directory where each world gets its own folder.
    Apart(PathBuf),
}

/// The human's choice of where `project`'s worlds go: `here` or `apart`.
pub fn choose_place(project: &Path, place: &str) -> io::Result<()> {
    if !matches!(place, "here" | "apart") {
        return Err(io::Error::other(format!("unknown place {place}")));
    }
    git(project, &["config", "fennec.workspace", place]).map(drop)
}

/// Where `project`'s worlds go: `here`, `apart`, or a path.
pub fn workspace(project: &Path) -> String {
    match git(project, &["config", "--get", "fennec.workspace"]) {
        Ok(chosen) if !chosen.is_empty() => chosen,
        _ => "here".to_string(),
    }
}

/// What keeps a new world out of a project whose worlds go in the folder itself.
pub enum Held {
    /// The world the folder is checked out on.
    World(String),
    /// The human's uncommitted changes: a failed attempt is undone by going
    /// back to the last commit, so nothing of theirs may be in the way.
    Changes,
}

/// Whether a new world must wait for `project`'s folder; never for worlds
/// that go apart.
pub fn held(project: &Path) -> io::Result<Option<Held>> {
    if !matches!(workspace(project).as_str(), "here") {
        return Ok(None);
    }
    let home = git(project, &["branch", "--show-current"])?;
    if let Some(id) = home.strip_prefix("world/") {
        return Ok(Some(Held::World(id.to_string())));
    }
    if !git(project, &["status", "--porcelain"])?.is_empty() {
        return Ok(Some(Held::Changes));
    }
    Ok(None)
}

/// A file the human changed through fennec (a rule in AGENTS.md) would hold
/// the folder until they commit it: where worlds go in the folder itself, it
/// is committed alone on their branch. Never on a world's branch, whose work
/// it would join.
pub fn keep(project: &Path, file: &str, message: &str) -> io::Result<()> {
    if workspace(project) != "here" {
        return Ok(());
    }
    // Not a repository yet: no world can hold it either.
    let Ok(branch) = git(project, &["branch", "--show-current"]) else {
        return Ok(());
    };
    // ponytail: during work in the folder the file stays uncommitted there
    // and joins that work, or goes with a failed attempt; commit it on the
    // human's branch with plumbing if that bites.
    if branch.is_empty() || branch.starts_with("world/") {
        return Ok(());
    }
    if git(project, &["status", "--porcelain", "--", file])?.is_empty() {
        return Ok(());
    }
    git(project, &["add", "--", file])?;
    git(project, &["commit", "-q", "-m", message, "--", file])?;
    Ok(())
}

/// Where a new world of `project` goes; see the module doc.
fn place(project: &Path) -> io::Result<Place> {
    let chosen = git(project, &["config", "--get", "fennec.workspace"]).unwrap_or_default();
    match chosen.as_str() {
        "" | "here" => {
            match held(project)? {
                Some(Held::World(id)) => {
                    return Err(io::Error::other(format!(
                        "the project folder already holds work in progress (world {id})"
                    )));
                }
                Some(Held::Changes) => {
                    return Err(io::Error::other(
                        "the project folder has uncommitted changes",
                    ));
                }
                None => {}
            }
            let home = git(project, &["branch", "--show-current"])?;
            if home.is_empty() {
                return Err(io::Error::other(
                    "the project folder is on no branch (detached HEAD)",
                ));
            }
            Ok(Place::Here(project.to_path_buf(), home.clone(), home))
        }
        "apart" => apart(project),
        dir => Ok(Place::Apart(PathBuf::from(dir))),
    }
}

/// Next to the project: `<parent>/<name>-fennec/`.
fn apart(project: &Path) -> io::Result<Place> {
    let project = std::fs::canonicalize(project)?;
    let name = project
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Place::Apart(
        project
            .parent()
            .unwrap_or(&project)
            .join(format!("{name}-fennec")),
    ))
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

/// Bytes under `dir`, symlinks not followed; 0 when it is gone.
fn size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}

/// Every file in the world is the work, but what a build writes is not:
/// a project without a .gitignore would land `target/` or `node_modules/`.
/// ponytail: a fixed list of tool outputs; a project that tracks one of
/// these dirs on purpose cannot change it through fennec.
const BUILD_OUTPUT: [&str; 6] = [
    ":(exclude,glob)**/target/**",
    ":(exclude,glob)**/node_modules/**",
    ":(exclude,glob)**/__pycache__/**",
    ":(exclude,glob)**/.venv/**",
    ":(exclude,glob)**/.gradle/**",
    ":(exclude,glob).fennec/**",
];

/// The package caches stay out of the human's `git status` and out of a
/// stash: listed in the store's own excludes, never in a file they commit.
fn ignore_cache(common: &Path) -> io::Result<()> {
    let info = common.join("info");
    let file = info.join("exclude");
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    if text.lines().any(|l| l.trim() == "/.fennec/") {
        return Ok(());
    }
    std::fs::create_dir_all(&info)?;
    let sep = if text.is_empty() || text.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    std::fs::write(&file, format!("{text}{sep}/.fennec/\n"))
}

/// Where a sandboxed command's package caches live, in the project.
const CACHE: &str = ".fennec";

/// The variable each package manager reads its cache from, and its folder
/// under [`CACHE`]. Maven takes a property ([`Worlds::jail`]).
const CACHES: [(&str, &str); 9] = [
    ("CARGO_HOME", "cargo"),
    // pnpm's store: its own prefix since pnpm 10, npm's before.
    ("npm_config_store_dir", "pnpm"),
    ("pnpm_config_store_dir", "pnpm"),
    ("UV_CACHE_DIR", "uv"),
    ("PIP_CACHE_DIR", "pip"),
    ("npm_config_cache", "npm"),
    ("GOMODCACHE", "go/mod"),
    ("GOCACHE", "go/build"),
    ("GRADLE_USER_HOME", "gradle"),
];

/// Without network, package managers use what the cache holds instead of
/// failing on a fetch.
const OFFLINE: [(&str, &str); 4] = [
    ("UV_OFFLINE", "1"),
    ("CARGO_NET_OFFLINE", "true"),
    ("npm_config_offline", "true"),
    ("GOPROXY", "off"),
];

/// Tools an agent may look for, named to it when the sandbox finds them.
const TOOLS: [&str; 30] = [
    "python3", "uv", "pip3", "poetry", "node", "npm", "pnpm", "yarn", "bun", "deno", "cargo",
    "rustc", "go", "java", "mvn", "gradle", "kotlinc", "dotnet", "gcc", "g++", "clang", "cmake",
    "make", "ninja", "meson", "php", "composer", "ruby", "bundle", "sqlite3",
];

/// Time-ordered ids: runs are named after their world, and "the latest run"
/// is the last id in lexical order. Wall time can step back (an NTP or WSL
/// host resync) and two forks can share a microsecond, so within a process
/// each id is strictly above the one before.
pub(crate) fn new_id() -> String {
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
    fn a_cargo_path_dependency_outside_the_project_is_found() {
        let root = std::env::temp_dir().join(format!("fennec-os-deps-{}", std::process::id()));
        let (app, lib) = (root.join("app"), root.join("lib"));
        std::fs::create_dir_all(app.join("inner")).unwrap();
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("Cargo.toml"), "").unwrap();
        // A folder that holds no crate is no dependency, whatever is written.
        std::fs::create_dir_all(root.join("secret")).unwrap();
        std::fs::write(
            app.join("Cargo.toml"),
            "[dependencies]\nlib = { path = \"../lib\" }\ninner = { path=\"inner\" }\ngone = { path = \"../gone\" }\nx = { path = \"../secret\" }\n",
        )
        .unwrap();
        let found = super::path_deps(&app);
        let lib = std::fs::canonicalize(&lib).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(found, vec![lib]);
    }

    #[test]
    fn ids_only_grow_even_within_one_microsecond() {
        let ids: Vec<String> = (0..10_000).map(|_| super::new_id()).collect();
        assert!(
            ids.windows(2)
                .all(|w| w[0] < w[1] && w[0].len() == w[1].len())
        );
    }
}
