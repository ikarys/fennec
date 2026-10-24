//! Grants: the human lets one world write named host paths. The harness backs
//! the paths up first and keeps the backup until the human keeps the changes
//! or reverts them. Records live in `<store>/grants/`, which no world reaches.
//! Files only: no socket, no command, no network.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::world::{World, Worlds};

fn dir(worlds: &Worlds, world: &World) -> PathBuf {
    worlds.root.join("grants").join(&world.id)
}

/// The paths granted to the world; empty when there is no grant.
pub fn paths(worlds: &Worlds, id: &str) -> io::Result<Vec<PathBuf>> {
    let Ok(text) = std::fs::read_to_string(worlds.root.join("grants").join(id).join("paths.json"))
    else {
        return Ok(Vec::new());
    };
    serde_json::from_str(&text).map_err(io::Error::other)
}

/// Backs up `paths` and grants them. Only paths inside the home are accepted,
/// never the home itself and never the store.
pub fn give(worlds: &Worlds, world: &World, paths: &[PathBuf]) -> io::Result<()> {
    if paths.is_empty() {
        return Err(io::Error::other("grant what? give at least one path"));
    }
    let d = dir(worlds, world);
    if d.join("paths.json").exists() {
        return Err(io::Error::other(
            "this world already has a grant: keep or revert it first",
        ));
    }
    // A directory without a record is what a killed `give` leaves behind.
    if d.exists() {
        std::fs::remove_dir_all(&d)?;
    }
    let home = std::fs::canonicalize(&worlds.home)?;
    let store = std::fs::canonicalize(&worlds.root)?;
    let mut granted = Vec::new();
    for p in paths {
        let p = std::fs::canonicalize(p)?;
        if !p.starts_with(&home) || p == home {
            return Err(io::Error::other(format!(
                "{} is not strictly inside {}",
                p.display(),
                home.display()
            )));
        }
        if p.starts_with(&store) || store.starts_with(&p) {
            return Err(io::Error::other(format!(
                "{} overlaps the world store",
                p.display()
            )));
        }
        granted.push(p);
    }
    std::fs::create_dir_all(&d)?;
    let backed_up = granted
        .iter()
        .enumerate()
        .try_for_each(|(i, p)| copy(p, &d.join(i.to_string())));
    let record = backed_up.and_then(|()| {
        let text = serde_json::to_string(&granted).map_err(io::Error::other)?;
        std::fs::write(d.join("paths.json"), text)
    });
    if record.is_err() {
        let _ = std::fs::remove_dir_all(&d);
    }
    record
}

/// Grants waiting for a decision: world id, paths, age; oldest first.
pub fn pending(worlds: &Worlds) -> io::Result<Vec<(String, Vec<PathBuf>, Duration)>> {
    let Ok(entries) = std::fs::read_dir(worlds.root.join("grants")) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for e in entries.filter_map(|e| e.ok()) {
        let id = e.file_name().to_string_lossy().into_owned();
        let Ok(record) = std::fs::metadata(e.path().join("paths.json")) else {
            continue; // incomplete: the next `give` clears it
        };
        let age = record.modified().ok().and_then(|m| m.elapsed().ok());
        out.push((id.clone(), paths(worlds, &id)?, age.unwrap_or_default()));
    }
    out.sort_by_key(|g| std::cmp::Reverse(g.2));
    Ok(out)
}

fn age(d: Duration) -> String {
    match d.as_secs() {
        s if s >= 86_400 => format!("{}d", s / 86_400),
        s if s >= 3_600 => format!("{}h", s / 3_600),
        s => format!("{}min", s / 60),
    }
}

/// One line per pending grant, for `grant list`.
pub fn list(worlds: &Worlds) -> io::Result<String> {
    Ok(pending(worlds)?
        .iter()
        .map(|(id, paths, d)| {
            let paths: Vec<_> = paths.iter().map(|p| p.display().to_string()).collect();
            format!("{id}  {}  {}\n", age(*d), paths.join(", "))
        })
        .collect())
}

/// The reminder: `None` when nothing waits.
pub fn summary(worlds: &Worlds) -> io::Result<Option<String>> {
    let pending = pending(worlds)?;
    Ok(pending.first().map(|(_, _, oldest)| {
        format!(
            "{} grant(s) waiting for keep or revert, oldest {}: fennec-os grant list",
            pending.len(),
            age(*oldest)
        )
    }))
}

/// Desktop notification through `notify-send` when it exists, else stderr.
/// Decides nothing: the human keeps or reverts.
pub fn remind(worlds: &Worlds) -> io::Result<()> {
    let Some(text) = summary(worlds)? else {
        return Ok(());
    };
    let body = format!("{text}\n{}", list(worlds)?);
    let sent = Command::new("notify-send")
        .args(["fennec-os", &body])
        .status()
        .is_ok_and(|s| s.success());
    if !sent {
        eprintln!("{body}");
    }
    Ok(())
}

/// What the world changed since the grant, as a unified diff.
pub fn diff(worlds: &Worlds, world: &World) -> io::Result<String> {
    let d = dir(worlds, world);
    let mut out = String::new();
    for (i, p) in paths(worlds, &world.id)?.iter().enumerate() {
        out.push_str(&format!("granted: {}\n", p.display()));
        let run = Command::new("diff")
            .args(["-ruN", "--no-dereference"])
            .arg(d.join(i.to_string()))
            .arg(p)
            .output()?;
        // 0: same, 1: different, more: diff itself failed.
        if run.status.code().is_none_or(|c| c > 1) {
            return Err(io::Error::other(
                String::from_utf8_lossy(&run.stderr).into_owned(),
            ));
        }
        out.push_str(&String::from_utf8_lossy(&run.stdout));
    }
    Ok(out)
}

/// The human accepts the changes: the backup goes, the grant ends.
pub fn keep(worlds: &Worlds, world: &World) -> io::Result<()> {
    std::fs::remove_dir_all(dir(worlds, world))
}

/// The human refuses the changes: each path is replaced by its backup.
/// A failure leaves the grant in place so that the revert can be retried.
pub fn revert(worlds: &Worlds, world: &World) -> io::Result<()> {
    let d = dir(worlds, world);
    for (i, p) in paths(worlds, &world.id)?.iter().enumerate() {
        // `symlink_metadata`, `remove_dir_all`: never follow a link the world planted.
        match std::fs::symlink_metadata(p) {
            Ok(m) if m.is_dir() => std::fs::remove_dir_all(p)?,
            Ok(_) => std::fs::remove_file(p)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        copy(&d.join(i.to_string()), p)?;
    }
    std::fs::remove_dir_all(d)
}

/// `cp -a`: links stay links, nothing is dereferenced; reflink on Btrfs.
fn copy(from: &Path, to: &Path) -> io::Result<()> {
    let status = Command::new("cp")
        .args(["-a", "--reflink=auto"])
        .arg(from)
        .arg(to)
        .status()?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| io::Error::other(format!("cannot copy {}", from.display())))
}
