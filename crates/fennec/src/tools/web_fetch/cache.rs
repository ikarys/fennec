//! The page cache: one Markdown file per URL, reused for a day, purged after
//! a week.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const FRESH_FOR: Duration = Duration::from_secs(24 * 3600);
const PURGE_AFTER: Duration = Duration::from_secs(7 * 24 * 3600);

/// `$XDG_CACHE_HOME/fennec/web`, else `$HOME/.cache/fennec/web`.
pub(crate) fn default_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("fennec").join("web")
}

pub(super) fn path_for(dir: &Path, url: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    dir.join(format!("{:016x}.md", hasher.finish()))
}

pub(super) fn is_fresh(path: &Path) -> bool {
    age(path).is_some_and(|age| age < FRESH_FOR)
}

/// Deletes the files older than a week.
// ponytail: age-only purge; add a total size cap if the cache grows.
pub(super) fn purge(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if age(&path).is_some_and(|age| age > PURGE_AFTER) {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn age(path: &Path) -> Option<Duration> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    SystemTime::now().duration_since(modified).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::test_support::temp_dir;
    use std::fs::File;

    fn age_file(path: &Path, days: u64) {
        let file = File::options().write(true).open(path).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(days * 24 * 3600 + 60))
            .unwrap();
    }

    #[test]
    fn the_name_is_stable_per_url() {
        let dir = Path::new("/c");
        assert_eq!(path_for(dir, "https://a"), path_for(dir, "https://a"));
        assert_ne!(path_for(dir, "https://a"), path_for(dir, "https://b"));
    }

    #[test]
    fn a_day_old_file_is_stale_and_a_week_old_one_is_purged() {
        let dir = temp_dir();
        let fresh = dir.join("fresh.md");
        let old = dir.join("old.md");
        let ancient = dir.join("ancient.md");
        for path in [&fresh, &old, &ancient] {
            std::fs::write(path, "x").unwrap();
        }
        age_file(&old, 2);
        age_file(&ancient, 8);

        assert!(is_fresh(&fresh));
        assert!(!is_fresh(&old));
        purge(&dir);
        assert!(fresh.exists() && old.exists() && !ancient.exists());
    }
}
