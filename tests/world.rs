//! The world's guarantees, checked against a real bubblewrap sandbox.

mod common;

use std::path::Path;
use std::process::Command;

use common::Fixture;
use fennec_os::grant;

#[test]
fn rollback_restores_the_last_checkpoint() {
    let f = Fixture::new("rollback");
    let w = f.worlds.fork(&f.project.to_string_lossy()).unwrap();
    assert!(f.sh(&w, "echo new > made && echo changed > README").0);
    let diff = f.worlds.diff(&w).unwrap();
    assert!(diff.contains("made") && diff.contains("changed"), "{diff}");

    f.worlds.rollback(&w).unwrap();
    assert!(!w.path.join("made").exists());
    assert_eq!(
        std::fs::read_to_string(w.path.join("README")).unwrap(),
        "hello\n"
    );
    assert!(f.worlds.diff(&w).unwrap().is_empty());
}

#[test]
fn a_folder_inside_a_repository_is_not_that_repository() {
    let f = Fixture::new("ceiling");
    // A project whose `.git` is gone, inside a folder that is a repository.
    let inner = f.project.join("inner");
    std::fs::create_dir(&inner).unwrap();
    let Err(e) = f.worlds.fork(&inner.to_string_lossy()) else {
        panic!("forked the parent repository");
    };
    let e = e.to_string();
    assert!(e.contains("not a git repository"), "{e}");
}

#[test]
fn running_python_leaves_no_bytecode_in_the_work() {
    let f = Fixture::new("bytecode");
    let w = f.worlds.fork(&f.project.to_string_lossy()).unwrap();
    assert!(
        f.sh(
            &w,
            "mkdir -p pkg && echo 'X = 1' > pkg/m.py && python3 -c 'import pkg.m'"
        )
        .0
    );
    let diff = f.worlds.diff(&w).unwrap();
    assert!(
        diff.contains("pkg/m.py") && !diff.contains("__pycache__"),
        "{diff}"
    );
}

#[test]
fn build_output_is_not_the_work() {
    let f = Fixture::new("build-output");
    let w = f.worlds.fork(&f.project.to_string_lossy()).unwrap();
    assert!(
        f.sh(
            &w,
            "mkdir -p target/debug node_modules/x && echo o > target/debug/a.o && echo j > node_modules/x/i.js && echo s > src.rs"
        )
        .0
    );
    let diff = f.worlds.diff(&w).unwrap();
    assert!(
        diff.contains("src.rs") && !diff.contains("target/") && !diff.contains("node_modules"),
        "{diff}"
    );
    f.worlds.commit(&w, "work").unwrap();
    let files = f.worlds.git(&w, &["ls-files"]).unwrap();
    assert!(
        !files.contains("target/") && files.contains("src.rs"),
        "{files}"
    );
}

#[test]
fn the_sandbox_cannot_reach_out() {
    let f = Fixture::new("isolation");
    let w = f.worlds.fork(&f.project.to_string_lossy()).unwrap();
    let project = f.project.to_string_lossy();
    let home = f.worlds.home.to_string_lossy();

    // Writes outside the world never reach the host.
    f.sh(
        &w,
        &format!("echo x > {project}/escaped; echo x > /tmp/escaped"),
    );
    assert!(!f.project.join("escaped").exists());
    assert!(!Path::new("/tmp/escaped").exists());
    // The home is hidden.
    assert!(!f.sh(&w, &format!("cat {home}/secret")).0);
    // No host socket: the session bus and daemons live under /run.
    assert_eq!(f.sh(&w, "ls -A /run").1, "");
    // Only loopback exists.
    assert_eq!(f.sh(&w, "awk 'NR>2{print $1}' /proc/net/dev").1, "lo:");
    // The agent cannot commit: the project's object store is read-only.
    assert!(
        !f.sh(
            &w,
            "git add -A && git -c user.name=a -c user.email=a@a commit -qm x --allow-empty"
        )
        .0
    );
    // The project's own branch is untouched.
    assert!(!f.project.join("made").exists());
}

#[test]
fn a_world_forks_from_its_checkpoint_and_drops_cleanly() {
    let f = Fixture::new("fork");
    let a = f.worlds.fork(&f.project.to_string_lossy()).unwrap();
    assert!(f.sh(&a, "echo kept > kept").0);
    f.worlds.commit(&a, "checkpoint").unwrap();
    assert!(f.sh(&a, "echo dropped > dropped").0);

    let b = f.worlds.fork(&a.id).unwrap();
    assert!(b.path.join("kept").exists());
    assert!(!b.path.join("dropped").exists());
    assert_eq!(f.worlds.list().unwrap().len(), 2);

    let b_id = b.id.clone();
    f.worlds.drop(b).unwrap();
    assert!(f.worlds.open(&b_id).is_err());
    let branches = Command::new("git")
        .arg("-C")
        .arg(&f.project)
        .args(["branch", "--list", "world/*"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&branches.stdout).lines().count(), 1);
}

#[test]
fn a_forged_git_file_grants_nothing() {
    let f = Fixture::new("forged");
    // Another repository on the host, whose store the agent covets.
    let victim = f.dir.join("victim");
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .arg(&victim)
            .status()
            .unwrap()
            .success()
    );
    let w = f.worlds.fork(&f.project.to_string_lossy()).unwrap();
    let victim_git = victim.join(".git");
    assert!(
        f.sh(
            &w,
            &format!("echo 'gitdir: {}' > .git", victim_git.display())
        )
        .0
    );
    // The harness still finds the world's real store, and mounts nothing new.
    assert!(
        !f.sh(
            &w,
            &format!("echo x > {}/hooks/post-commit", victim_git.display())
        )
        .0
    );
    assert!(!victim_git.join("hooks/post-commit").exists());
    f.worlds.commit(&w, "checkpoint").unwrap();
    f.worlds.drop(w).unwrap();
}

#[test]
fn harness_git_never_runs_hooks_the_agent_wrote() {
    let f = Fixture::new("hooks");
    let canary = f.dir.join("canary");
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&f.project)
            .args(["config", "core.hooksPath", ".hooks"])
            .status()
            .unwrap()
            .success()
    );
    let w = f.worlds.fork(&f.project.to_string_lossy()).unwrap();
    let hook = format!(
        "mkdir .hooks && printf '#!/bin/sh\\ntouch {}\\n' > .hooks/pre-commit && chmod +x .hooks/pre-commit",
        canary.display()
    );
    assert!(f.sh(&w, &hook).0);
    f.worlds.commit(&w, "checkpoint").unwrap();
    assert!(!canary.exists());
}

#[test]
fn the_agent_can_still_read_git_history() {
    let f = Fixture::new("gitread");
    let w = f.worlds.fork(&f.project.to_string_lossy()).unwrap();
    assert!(
        f.sh(
            &w,
            "echo more >> README && git status --short && git diff --stat && git log --oneline"
        )
        .0
    );
}

#[test]
fn a_command_runs_under_the_memory_cap() {
    let mut f = Fixture::new("memory-cap");
    let w = f.worlds.fork(&f.project.to_string_lossy()).unwrap();
    f.worlds.memory_mb = Some(64);
    assert_eq!(f.sh(&w, "ulimit -v"), (true, "65536".to_string()));
    // Arguments still reach the command intact.
    assert_eq!(f.sh(&w, "echo \"$0\""), (true, "sh".to_string()));
    f.worlds.memory_mb = None;
    assert_eq!(f.sh(&w, "ulimit -v"), (true, "unlimited".to_string()));
}

#[test]
fn a_grant_lets_a_world_write_a_host_path_until_the_human_decides() {
    let f = Fixture::new("grant");
    let w = f.worlds.fork(&f.project.to_string_lossy()).unwrap();
    let conf = f.worlds.home.join("conf");
    std::fs::create_dir_all(&conf).unwrap();
    std::fs::write(conf.join("app.cfg"), "v1\n").unwrap();
    let cfg = || std::fs::read_to_string(conf.join("app.cfg")).unwrap();

    // Without a grant the host path is not even there.
    assert!(!f.sh(&w, &format!("echo x > {}/app.cfg", conf.display())).0);
    assert_eq!(cfg(), "v1\n");

    // The home, the store and paths outside the home are refused.
    for bad in [&f.worlds.home, &f.worlds.root, &f.dir] {
        assert!(grant::give(&f.worlds, &w, std::slice::from_ref(bad)).is_err());
    }
    grant::give(&f.worlds, &w, std::slice::from_ref(&conf)).unwrap();
    assert!(grant::give(&f.worlds, &w, std::slice::from_ref(&conf)).is_err());
    assert!(
        f.sh(
            &w,
            &format!("echo v2 > {0}/app.cfg && echo n > {0}/new", conf.display())
        )
        .0
    );
    assert_eq!(cfg(), "v2\n");
    let diff = grant::diff(&f.worlds, &w).unwrap();
    assert!(
        diff.contains("-v1") && diff.contains("+v2") && diff.contains("new"),
        "{diff}"
    );
    // A world with a grant is not dropped behind the human's back.
    assert!(f.worlds.drop(f.worlds.open(&w.id).unwrap()).is_err());

    grant::revert(&f.worlds, &w).unwrap();
    assert_eq!(cfg(), "v1\n");
    assert!(!conf.join("new").exists());
    // The grant is over: the path is out of reach again.
    assert!(!f.sh(&w, &format!("echo x > {}/app.cfg", conf.display())).0);

    grant::give(&f.worlds, &w, std::slice::from_ref(&conf)).unwrap();
    assert!(f.sh(&w, &format!("echo v3 > {}/app.cfg", conf.display())).0);
    grant::keep(&f.worlds, &w).unwrap();
    assert_eq!(cfg(), "v3\n");
    assert!(grant::paths(&f.worlds, &w.id).unwrap().is_empty());
    f.worlds.drop(w).unwrap();
}

#[test]
fn pending_grants_are_listed_and_a_killed_give_is_cleared() {
    let f = Fixture::new("grant-pending");
    let w = f.worlds.fork(&f.project.to_string_lossy()).unwrap();
    let conf = f.worlds.home.join("conf");
    std::fs::create_dir_all(&conf).unwrap();
    assert_eq!(grant::summary(&f.worlds).unwrap(), None);

    // What a `give` killed after its backup, before its record, leaves behind.
    let stale = f.worlds.root.join("grants").join(&w.id);
    std::fs::create_dir_all(stale.join("0")).unwrap();
    assert_eq!(grant::summary(&f.worlds).unwrap(), None);
    grant::give(&f.worlds, &w, std::slice::from_ref(&conf)).unwrap();

    let list = grant::list(&f.worlds).unwrap();
    assert!(list.starts_with(&w.id) && list.contains("conf"), "{list}");
    let summary = grant::summary(&f.worlds).unwrap().unwrap();
    assert!(summary.starts_with("1 grant(s)"), "{summary}");
    grant::keep(&f.worlds, &w).unwrap();
    assert_eq!(grant::summary(&f.worlds).unwrap(), None);
}
