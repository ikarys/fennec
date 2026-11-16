//! Finds the security scanners of `domain::scanner` on PATH, as the sandbox sees it (commands inherit fennec's environment).

use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// The scanner binaries found on PATH, cached per binary: the PATH and the
/// installed binaries do not change during a session, and a WSL `/mnt/c` PATH
/// is slow to stat, so re-searching it on every settings read would lag saves.
static FOUND: LazyLock<std::sync::Mutex<HashMap<String, Option<PathBuf>>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

pub fn find(binary: &str) -> Option<PathBuf> {
    let mut cache = crate::locks::lock(&FOUND);
    if let Some(cached) = cache.get(binary) {
        return cached.clone();
    }
    let found = std::env::var_os("PATH").and_then(|path| find_in(&path, binary));
    cache.insert(binary.to_string(), found.clone());
    found
}

fn find_in(path: &OsStr, binary: &str) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|dir| dir.join(binary))
        .find(|candidate| is_executable_file(candidate))
}

fn is_executable_file(candidate: &Path) -> bool {
    candidate
        .metadata()
        .map(|metadata| metadata.is_file() && (metadata.permissions().mode() & 0o111) != 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let counter = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fennec-scanners-{}-{counter}-{nanos}",
            std::process::id()
        ))
    }

    fn set_mode(path: &Path, mode: u32) {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
            .expect("set permissions");
    }

    #[test]
    fn finds_an_executable_file() {
        let root = temp_root();
        std::fs::create_dir_all(&root).expect("create temp dir");
        let binary = root.join("scanner");
        std::fs::write(&binary, b"").expect("write binary");
        set_mode(&binary, 0o755);

        let path = std::env::join_paths([root.clone()]).expect("join paths");
        let found = find_in(&path, "scanner").expect("find executable");
        assert_eq!(found, binary);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn does_not_find_a_non_executable_file() {
        let root = temp_root();
        std::fs::create_dir_all(&root).expect("create temp dir");
        let binary = root.join("scanner");
        std::fs::write(&binary, b"").expect("write binary");
        set_mode(&binary, 0o644);

        let path = std::env::join_paths([root.clone()]).expect("join paths");
        assert_eq!(find_in(&path, "scanner"), None);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn does_not_find_an_absent_binary() {
        let root = temp_root();
        std::fs::create_dir_all(&root).expect("create temp dir");

        let path = std::env::join_paths([root.clone()]).expect("join paths");
        assert_eq!(find_in(&path, "scanner"), None);

        let _ = std::fs::remove_dir_all(&root);
    }

    /// PATH is process-wide; the test that rewrites it runs alone, restoring it after.
    static PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn find_caches_so_a_removed_binary_is_still_found() {
        let _guard = PATH_LOCK.lock().unwrap();
        let name = format!("fennec-cache-scanner-{}", std::process::id());
        let root = temp_root();
        std::fs::create_dir_all(&root).expect("create temp dir");
        let binary = root.join(&name);
        std::fs::write(&binary, b"").expect("write binary");
        set_mode(&binary, 0o755);

        let old_path = std::env::var_os("PATH");
        unsafe { std::env::set_var("PATH", &root) };

        let first = find(&name).expect("find the binary");
        assert_eq!(first, binary);

        // A fresh search would now fail: the binary is gone. The cache keeps it.
        std::fs::remove_file(&binary).expect("remove binary");
        let second = find(&name).expect("the cached path, not a fresh search");
        assert_eq!(second, binary);

        unsafe {
            match old_path {
                Some(path) => std::env::set_var("PATH", path),
                None => std::env::remove_var("PATH"),
            }
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn first_directory_in_path_order_wins() {
        let root = temp_root();
        let first = root.join("first");
        let second = root.join("second");
        std::fs::create_dir_all(&first).expect("create first");
        std::fs::create_dir_all(&second).expect("create second");
        let first_binary = first.join("scanner");
        let second_binary = second.join("scanner");
        std::fs::write(&first_binary, b"").expect("write first");
        std::fs::write(&second_binary, b"").expect("write second");
        set_mode(&first_binary, 0o755);
        set_mode(&second_binary, 0o755);

        let path = std::env::join_paths([first.clone(), second.clone()]).expect("join paths");
        let found = find_in(&path, "scanner").expect("find executable");
        assert_eq!(found, first_binary);

        let _ = std::fs::remove_dir_all(&root);
    }
}
