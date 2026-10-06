//! Priorities between fennec-os processes sharing one local model: background
//! work (sleep) yields to interactive work (solve). Advisory file locks, so
//! the order holds across processes and dies with them.
//!
//! ponytail: only fennec-os's own load is visible; other clients of the model
//! server are not (this server exposes no load metrics). Add a load probe when
//! the server has one.

use std::fs::{File, TryLockError};
use std::io;

use crate::world::Worlds;

fn lock_file(worlds: &Worlds) -> io::Result<File> {
    std::fs::create_dir_all(&worlds.root)?;
    File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(worlds.root.join("interactive.lock"))
}

/// Held for the length of interactive work; many may be held at once.
pub struct Interactive(#[allow(dead_code)] File);

pub fn interactive(worlds: &Worlds) -> io::Result<Interactive> {
    let file = lock_file(worlds)?;
    file.lock_shared()?;
    Ok(Interactive(file))
}

/// Blocks while any interactive work runs. Returns whether it had to wait.
pub fn yield_to_interactive(worlds: &Worlds) -> io::Result<bool> {
    let file = lock_file(worlds)?;
    match file.try_lock() {
        Ok(()) => Ok(false),
        Err(TryLockError::WouldBlock) => {
            file.lock()?;
            Ok(true)
        }
        Err(TryLockError::Error(e)) => Err(e),
    }
    // The exclusive lock is released when `file` drops: background work
    // checks again before each model call, it does not hold the line.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn background_waits_for_interactive_work() {
        let dir = std::env::temp_dir().join(format!("fennec-os-sched-{}", std::process::id()));
        let worlds = Worlds {
            root: dir.clone(),
            home: dir.clone(),
            ro_paths: Vec::new(),
            memory_mb: None,
        };
        assert!(!yield_to_interactive(&worlds).unwrap());
        let a = interactive(&worlds).unwrap();
        let b = interactive(&worlds).unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::scope(|s| {
            s.spawn(|| tx.send(yield_to_interactive(&worlds).unwrap()).unwrap());
            assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
            drop(a);
            assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
            drop(b);
            assert!(rx.recv_timeout(Duration::from_secs(5)).unwrap());
        });
        let _ = std::fs::remove_dir_all(dir);
    }
}
