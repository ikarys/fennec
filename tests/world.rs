//! The world's guarantees, checked against a real bubblewrap sandbox.

mod common;

use std::path::Path;
use std::process::Command;

use common::Fixture;

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
