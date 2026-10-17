//! Shared fixture: a temp git project, a fake home, a world store.

use std::path::PathBuf;
use std::process::Command;

use fennec_os::world::{World, Worlds};

pub struct Fixture {
    pub dir: PathBuf,
    #[allow(dead_code)]
    pub project: PathBuf,
    pub worlds: Worlds,
}

impl Fixture {
    pub fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("fennec-os-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let project = dir.join("project");
        let home = dir.join("home");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("secret"), "s3cret").unwrap();
        std::fs::write(project.join("README"), "hello\n").unwrap();
        for args in [
            &["init", "-q", "-b", "main"][..],
            // The human's identity: landed work is theirs.
            &["config", "user.name", "Human"],
            &["config", "user.email", "human@example.com"],
            &["add", "-A"],
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-q",
                "-m",
                "init",
            ],
        ] {
            assert!(
                Command::new("git")
                    .arg("-C")
                    .arg(&project)
                    .args(args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        // Worlds apart, in the store, unless a test asks for the folder.
        let store = dir.join("store");
        let apart = store.join("worlds");
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&project)
                .args(["config", "fennec.workspace"])
                .arg(&apart)
                .status()
                .unwrap()
                .success()
        );
        let worlds = Worlds {
            root: store,
            home,
            ro_paths: Vec::new(),
            memory_mb: Some(8192),
        };
        Self {
            dir,
            project,
            worlds,
        }
    }

    #[allow(dead_code)]
    pub fn sh(&self, world: &World, script: &str) -> (bool, String) {
        let argv = ["sh".to_string(), "-c".to_string(), script.to_string()];
        let out = self
            .worlds
            .sandbox_command(world, &argv)
            .unwrap()
            .output()
            .unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
