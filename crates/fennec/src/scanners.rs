//! Finds the security scanners of `domain::scanner` on PATH, as the sandbox sees it (commands inherit fennec's environment).

use crate::executable;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

/// The scanner binaries found on PATH. A WSL `/mnt/c` PATH is slow to scan, so
/// a found binary is kept and only re-checked with one stat; a missing one is
/// not cached, so a scanner installed while fennec runs is seen at once.
static FOUND: LazyLock<Mutex<HashMap<String, PathBuf>>> = LazyLock::new(Mutex::default);

pub fn find(binary: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    find_cached(&FOUND, &path, binary)
}

/// `binary` from `cache` while it is still an executable file, else searched in
/// `path`; the lock is not held during the search.
fn find_cached(
    cache: &Mutex<HashMap<String, PathBuf>>,
    path: &OsStr,
    binary: &str,
) -> Option<PathBuf> {
    let cached = crate::locks::lock(cache).get(binary).cloned();
    if let Some(found) = cached.filter(|found| executable::is_executable_file(found)) {
        return Some(found);
    }
    let found = executable::find_in(path, binary);
    let mut cache = crate::locks::lock(cache);
    match &found {
        Some(found) => cache.insert(binary.to_string(), found.clone()),
        None => cache.remove(binary),
    };
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn temp_root() -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let counter = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!("fennec-scanners-{}-{counter}", std::process::id()))
    }

    fn write_executable(path: &std::path::Path) {
        std::fs::write(path, b"").expect("write binary");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .expect("set permissions");
    }

    #[test]
    fn a_scanner_installed_later_is_found_and_a_removed_one_is_not() {
        let root = temp_root();
        std::fs::create_dir_all(&root).expect("create temp dir");
        let path = std::env::join_paths([root.clone()]).expect("join paths");
        let binary = root.join("scanner");
        let cache = Mutex::default();

        assert_eq!(find_cached(&cache, &path, "scanner"), None);

        write_executable(&binary);
        assert_eq!(find_cached(&cache, &path, "scanner"), Some(binary.clone()));

        std::fs::remove_file(&binary).expect("remove binary");
        assert_eq!(find_cached(&cache, &path, "scanner"), None);
        assert!(crate::locks::lock(&cache).is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_found_scanner_is_served_from_the_cache() {
        let root = temp_root();
        std::fs::create_dir_all(&root).expect("create temp dir");
        let binary = root.join("scanner");
        write_executable(&binary);
        let cache = Mutex::new(HashMap::from([("scanner".to_string(), binary.clone())]));

        // An empty PATH: only the cache can answer.
        assert_eq!(find_cached(&cache, OsStr::new(""), "scanner"), Some(binary));

        let _ = std::fs::remove_dir_all(&root);
    }
}
