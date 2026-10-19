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
}

pub struct World {
    pub id: String,
    pub path: PathBuf,
}

/// The folders a Cargo project's `path = "..."` dependencies point to, outside it.
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
        .filter(|p| p.is_dir() && !p.starts_with(&dir))
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
                let place = place(project)?;
                (
                    PathBuf::from(common),
                    git(project, &["rev-parse", "HEAD"])?,
                    place,
                )
            }
        };
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
        mounts.push((false, common.clone(), common));
        self.jail(&world.path, &mounts, net, argv)
    }

    /// The command that runs `argv` in a sandbox over a folder of the
    /// human's that is no world: the same isolation, `dir` read-write,
    /// `grants` — host folders the human opened for the chat — read-write,
    /// and `skills` read-only. Only for a folder they chose (`chat`).
    pub fn sandbox_folder(
        &self,
        dir: &Path,
        skills: &[(String, PathBuf)],
        grants: &[PathBuf],
        argv: &[String],
    ) -> io::Result<Command> {
        let mut mounts = self.skill_mounts(skills);
        for g in grants {
            mounts.push((true, g.clone(), g.clone()));
        }
        mounts.push((true, dir.to_path_buf(), dir.to_path_buf()));
        self.jail(dir, &mounts, false, argv)
    }

    /// Skills, read-only, where the brief says they are.
    fn skill_mounts(&self, skills: &[(String, PathBuf)]) -> Vec<(bool, PathBuf, PathBuf)> {
        skills
            .iter()
            .filter(|(name, dir)| !name.contains('/') && dir.is_dir())
            .map(|(name, dir)| (false, dir.clone(), self.home.join(".skills").join(name)))
            .collect()
    }

    /// bwrap over `mounts` (read-write?, host path, sandbox path), in order.
    fn jail(
        &self,
        dir: &Path,
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
        for p in path_deps(dir) {
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

    /// Back to the last checkpoint, exactly: ignored files go too, except in
    /// the project folder, where they are the human's (`.env`, `.venv`).
    pub fn rollback(&self, world: &World) -> io::Result<()> {
        if self.home(world)?.is_some() {
            // The human may have edited their folder while fennec worked: what
            // goes is set aside in the stash, never lost.
            let mut args = COMMITTER.to_vec();
            args.extend(["stash", "push", "-q", "-u", "-m", "fennec: essai annulé"]);
            self.git(world, &args)?;
            return Ok(());
        }
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
        "apart" => {
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
        dir => Ok(Place::Apart(PathBuf::from(dir))),
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
        std::fs::write(
            app.join("Cargo.toml"),
            "[dependencies]\nlib = { path = \"../lib\" }\ninner = { path=\"inner\" }\ngone = { path = \"../gone\" }\n",
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
