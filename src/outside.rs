//! Out-of-folder references: the literal host paths a chat tool call touches
//! outside the folder the person chose. Only literals are seen: a path behind
//! a variable, a command substitution or a glob stays under the sandbox's
//! read-only host, not under a question.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

/// The host paths `tool`'s `args` reference outside `folder`; empty when the
/// call stays inside. `folder` is canonical; `home` is the sandbox's hidden
/// home, so `~` and `$HOME` in a command resolve to it; `ro_paths` are the
/// toolchains re-exposed in it.
pub fn outside(
    folder: &Path,
    home: &Path,
    ro_paths: &[PathBuf],
    tool: &str,
    args: &Value,
) -> Vec<PathBuf> {
    let candidates = match tool {
        "write_file" => args
            .get("path")
            .and_then(Value::as_str)
            .into_iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>(),
        "shell" => args
            .get("command")
            .and_then(Value::as_str)
            .map(tokens)
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let mut found = Vec::new();
    for raw in candidates {
        let Some(p) = resolve(&raw, folder, home) else {
            continue;
        };
        if covers(folder, &p) || local(&p, home, ro_paths) {
            continue;
        }
        found.push(p);
    }
    found.dedup();
    found
}

/// A grant on folder `g` covers `p` when `p` is `g` itself or inside it.
pub fn covers(g: &Path, p: &Path) -> bool {
    p == g || p.starts_with(g)
}

/// The folder to grant for a referenced `p`: `p` itself when it is a
/// directory, its parent otherwise (a file's folder is what stays open),
/// symlinks resolved.
pub fn grant_dir(p: &Path) -> PathBuf {
    let dir = if p.is_dir() {
        p.to_path_buf()
    } else {
        p.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| p.to_path_buf())
    };
    // The sandbox mounts where a symlink leads: that folder is the one asked
    // about and checked, never the harmless-looking link to the home.
    std::fs::canonicalize(&dir).unwrap_or(dir)
}

/// `~`, `$HOME` and relative references resolve against the folder; `..` is
/// folded lexically, so it cannot slip past the root it came from. Symlinks
/// are not resolved: the question is about the path as written.
fn resolve(raw: &str, folder: &Path, home: &Path) -> Option<PathBuf> {
    let abs = match raw {
        "~" => home.to_path_buf(),
        r"$HOME" => home.to_path_buf(),
        _ if let Some(rest) = raw.strip_prefix("~/") => home.join(rest),
        _ if let Some(rest) = raw.strip_prefix("$HOME/") => home.join(rest),
        _ if raw.starts_with('/') => PathBuf::from(raw),
        _ => folder.join(raw),
    };
    Some(lexicalize(&abs))
}

fn lexicalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c.as_os_str()),
        }
    }
    out
}

/// A reference is not a host path when the sandbox already serves it: the
/// private /dev, /proc, /tmp, /run, the toolchains re-exposed in the hidden
/// home, and the skills mounted there. Everything else under the hidden home
/// is invisible in the sandbox, so it is exactly what a question is for.
fn local(p: &Path, home: &Path, ro_paths: &[PathBuf]) -> bool {
    ["/dev", "/proc", "/tmp", "/run"]
        .iter()
        .any(|r| p.starts_with(r))
        || ro_paths.iter().any(|r| p.starts_with(r))
        || p.starts_with(home.join(".skills"))
}

/// Command tokens, quote-aware: a path in `"…"` or `'…'` is one token.
fn tokens(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in command.chars() {
        match (quote, ch) {
            (Some(q), ch) if q == ch => quote = None,
            (Some(_), ch) => cur.push(ch),
            (None, c @ ('\'' | '"')) => quote = Some(c),
            (None, c) if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.into_iter()
        .filter_map(|t| {
            // `--file=/x` names its path after the `=`; a bare flag names none.
            let p = t
                .strip_prefix("--")
                .and_then(|r| r.split_once('=').map(|(_, v)| v.to_string()))
                .unwrap_or(t);
            if is_path_token(&p) { Some(p) } else { None }
        })
        .collect()
}

fn is_path_token(t: &str) -> bool {
    t.starts_with('/')
        || t.starts_with('~')
        || t == ".."
        || t.starts_with("./")
        || t.starts_with("../")
        || t == "$HOME"
        || t.starts_with("$HOME/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn setup() -> (PathBuf, PathBuf, Vec<PathBuf>) {
        let home = PathBuf::from("/home/t");
        let folder = home.join("ws/proj");
        (folder, home, Vec::new())
    }

    #[test]
    fn a_call_inside_the_folder_names_nothing() {
        let (folder, home, ro) = setup();
        for cmd in [
            "cat a.txt",
            "ls src/../src",
            "cat /home/t/ws/proj/x.txt",
            "grep -r x .",
            "cat /dev/null",
            "mktemp -t",
            "cat /tmp/x /proc/1/cmdline /run/x /dev/shm/x",
            "cat ~/.skills/demo/SKILL.md",
        ] {
            assert_eq!(
                outside(&folder, &home, &ro, "shell", &json!({"command": cmd})),
                Vec::<PathBuf>::new(),
                "{cmd}"
            );
        }
        assert_eq!(
            outside(
                &folder,
                &home,
                &ro,
                "write_file",
                &json!({"path": "b.txt", "content": ""})
            ),
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn an_out_of_folder_path_is_named_with_its_lexical_home() {
        let (folder, home, ro) = setup();
        let got = outside(
            &folder,
            &home,
            &ro,
            "shell",
            &json!({"command": "cat /home/t/ws/other/o.txt"}),
        );
        assert_eq!(got, vec![PathBuf::from("/home/t/ws/other/o.txt")]);
        // `..` folded lexically: the folder it escapes to is named.
        let got = outside(
            &folder,
            &home,
            &ro,
            "shell",
            &json!({"command": "cat ../../ws/other/o.txt"}),
        );
        assert_eq!(got, vec![PathBuf::from("/home/t/ws/other/o.txt")]);
        // `~` is the hidden home.
        let got = outside(
            &folder,
            &home,
            &ro,
            "shell",
            &json!({"command": "cat ~/ws/other/o.txt"}),
        );
        assert_eq!(got, vec![PathBuf::from("/home/t/ws/other/o.txt")]);
        // A quoted path is one token.
        let got = outside(
            &folder,
            &home,
            &ro,
            "shell",
            &json!({"command": "cat \"/home/t ws/other/o.txt\""}),
        );
        assert_eq!(got, vec![PathBuf::from("/home/t ws/other/o.txt")]);
        // `--file=/x` names its path.
        let got = outside(
            &folder,
            &home,
            &ro,
            "shell",
            &json!({"command": "tool --file=/home/t/ws/other/o.txt"}),
        );
        assert_eq!(got, vec![PathBuf::from("/home/t/ws/other/o.txt")]);
    }

    #[test]
    fn a_sibling_of_the_hidden_home_is_named_and_not_a_re_exposed_toolchain() {
        let (folder, home, ro) = setup();
        // Under the hidden home but not re-exposed: invisible in the sandbox.
        let got = outside(
            &folder,
            &home,
            &ro,
            "shell",
            &json!({"command": "cat ~/notes/x"}),
        );
        assert_eq!(got, vec![PathBuf::from("/home/t/notes/x")]);
        let ro_paths = vec![home.join(".cargo")];
        let got = outside(
            &folder,
            &home,
            &ro_paths,
            "shell",
            &json!({"command": "cat ~/.cargo/config"}),
        );
        assert_eq!(got, Vec::<PathBuf>::new());
    }

    #[test]
    fn the_home_variable_is_the_hidden_home() {
        let (folder, home, ro) = setup();
        // Bare `$HOME` is the hidden home itself: invisible in the sandbox.
        let got = outside(
            &folder,
            &home,
            &ro,
            "shell",
            &json!({"command": "ls $HOME"}),
        );
        assert_eq!(got, vec![home.clone()]);
        let got = outside(
            &folder,
            &home,
            &ro,
            "shell",
            &json!({"command": "cat $HOME/notes/x"}),
        );
        assert_eq!(got, vec![PathBuf::from("/home/t/notes/x")]);
        // A `$HOME` reference inside the folder is in-folder.
        let got = outside(
            &folder,
            &home,
            &ro,
            "shell",
            &json!({"command": "cat $HOME/ws/proj/x"}),
        );
        assert_eq!(got, Vec::<PathBuf>::new());
    }

    #[test]
    fn write_file_names_its_path() {
        let (folder, home, ro) = setup();
        let got = outside(
            &folder,
            &home,
            &ro,
            "write_file",
            &json!({"path": "../other/n.txt", "content": "x"}),
        );
        assert_eq!(got, vec![PathBuf::from("/home/t/ws/other/n.txt")]);
        let got = outside(
            &folder,
            &home,
            &ro,
            "write_file",
            &json!({"path": "/home/t/ws/other", "content": "x"}),
        );
        assert_eq!(got, vec![PathBuf::from("/home/t/ws/other")]);
    }

    #[test]
    fn a_grant_covers_its_folder_and_not_its_siblings() {
        let grant = PathBuf::from("/home/t/ws/other");
        assert!(covers(&grant, &PathBuf::from("/home/t/ws/other")));
        assert!(covers(
            &grant,
            &PathBuf::from("/home/t/ws/other/sub/deep.txt")
        ));
        assert!(!covers(&grant, &PathBuf::from("/home/t/ws/other2/x")));
        assert!(!covers(&grant, &PathBuf::from("/home/t/ws/other-x")));
    }

    #[test]
    fn the_grant_dir_of_a_file_is_its_folder() {
        let p = PathBuf::from("/home/t/ws/other/o.txt");
        assert_eq!(grant_dir(&p), PathBuf::from("/home/t/ws/other"));
        let d = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        assert_eq!(grant_dir(&d), d);
    }

    #[test]
    fn a_symlink_grants_the_folder_it_leads_to() {
        let root = std::env::temp_dir().join(format!("fennec-os-link-{}", std::process::id()));
        let target = root.join("target");
        std::fs::create_dir_all(&target).unwrap();
        let link = root.join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let got = grant_dir(&link.join("x.txt"));
        let want = std::fs::canonicalize(&target).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(got, want);
    }
}
