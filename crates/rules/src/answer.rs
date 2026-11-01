//! The proposal fennec offers when it asks the human.

use serde::Serialize;

use crate::decide::Request;
use crate::rule::{Access, TargetKind};

/// The rule the "for the session", "always", and "never" answers would store.
///
/// The "once" answer stores nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub kind: TargetKind,
    pub pattern: String,
    pub access: Option<Access>,
}

/// Builds the narrowest useful rule for a request.
pub fn proposal(request: &Request) -> Proposal {
    match request {
        Request::Tool(name) => Proposal {
            kind: TargetKind::Tool,
            pattern: name.to_string(),
            access: None,
        },
        Request::Command(command) => Proposal {
            kind: TargetKind::Command,
            // Exact Allow matching keeps this narrow; trimming avoids shell whitespace
            // becoming part of the stored pattern.
            pattern: command.trim().to_string(),
            access: None,
        },
        Request::Path(path) => Proposal {
            kind: TargetKind::Path,
            pattern: path_parent_pattern(path),
            access: Some(path.access),
        },
    }
}

fn path_parent_pattern(path: &crate::decide::PathRequest) -> String {
    if let Some(relative) = &path.relative {
        return relative_parent(relative);
    }

    // `/home/u2/x` is not under `/home/u`: the rest must start at a separator.
    if let Some(home) = &path.home
        && let Some(rest) = path.absolute.strip_prefix(home.trim_end_matches('/'))
        && let Some(rest) = rest.strip_prefix('/')
        && !rest.is_empty()
    {
        return format!("~/{}", relative_parent(rest));
    }

    absolute_parent(&path.absolute)
}

fn relative_parent(relative: &str) -> String {
    let trimmed = relative.trim_end_matches('/');
    if trimmed.is_empty() {
        // The project root itself would cover everything; use a visible root marker.
        return ".".to_string();
    }
    if let Some(pos) = trimmed.rfind('/') {
        format!("{}/", &trimmed[..pos])
    } else {
        // A file directly in the project root becomes the file itself.
        trimmed.to_string()
    }
}

fn absolute_parent(absolute: &str) -> String {
    let trimmed = absolute.trim_end_matches('/');
    if trimmed.is_empty() {
        return "/".to_string();
    }
    if let Some(pos) = trimmed.rfind('/') {
        if pos == 0 {
            // A file directly under `/` becomes the file itself.
            trimmed.to_string()
        } else {
            format!("{}/", &trimmed[..pos])
        }
    } else {
        trimmed.to_string()
    }
}

/// The folders a human may grant instead of a proposed folder pattern: the
/// pattern itself, then each ancestor, narrowest first. A file pattern has no
/// broader choice.
pub fn broader_folders(pattern: &str) -> Vec<String> {
    let mut folders = vec![pattern.to_string()];
    if !pattern.ends_with('/') {
        return folders;
    }

    let mut current = pattern.to_string();
    while let Some(ancestor) = parent_folder(&current) {
        current = ancestor.to_string();
        folders.push(current.clone());
    }
    folders
}

fn parent_folder(folder: &str) -> Option<&str> {
    // Keep the trailing slash so every offered choice stays a folder pattern.
    let prefix = folder.strip_suffix('/')?;
    let position = prefix.rfind('/')?;
    let ancestor = &folder[..=position];
    // The roots would match almost everything, so they are never offered as a broader choice.
    if ancestor == "/" || ancestor == "~/" || ancestor == "./" {
        return None;
    }
    Some(ancestor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decide::PathRequest;

    #[test]
    fn broader_folders_returns_ancestors_narrowest_first() {
        assert_eq!(
            broader_folders("~/workspace/fennec/web/src/lib/"),
            vec![
                "~/workspace/fennec/web/src/lib/",
                "~/workspace/fennec/web/src/",
                "~/workspace/fennec/web/",
                "~/workspace/fennec/",
                "~/workspace/",
            ]
        );
    }

    #[test]
    fn broader_folders_stops_before_absolute_root() {
        assert_eq!(
            broader_folders("/opt/a/b/"),
            vec!["/opt/a/b/", "/opt/a/", "/opt/"]
        );
    }

    #[test]
    fn broader_folders_stops_before_relative_root() {
        assert_eq!(broader_folders("src/lib/"), vec!["src/lib/", "src/"]);
        assert_eq!(broader_folders("src/"), vec!["src/"]);
    }

    #[test]
    fn broader_folders_for_non_folder_returns_self() {
        assert_eq!(
            broader_folders("src/lib/file.txt"),
            vec!["src/lib/file.txt"]
        );
        assert_eq!(broader_folders("."), vec!["."]);
    }

    #[test]
    fn tool_proposal() {
        let proposal = proposal(&Request::Tool("web_search"));
        assert_eq!(
            proposal,
            Proposal {
                kind: TargetKind::Tool,
                pattern: "web_search".into(),
                access: None,
            }
        );
    }

    #[test]
    fn command_proposal_trims() {
        let proposal = proposal(&Request::Command("  git push origin main  "));
        assert_eq!(
            proposal,
            Proposal {
                kind: TargetKind::Command,
                pattern: "git push origin main".into(),
                access: None,
            }
        );
    }

    #[test]
    fn path_proposal_inside_project_root_file() {
        let path = PathRequest {
            absolute: "/project/file.txt".into(),
            relative: Some("file.txt".into()),
            home: None,
            access: Access::Read,
        };
        let proposal = proposal(&Request::Path(&path));
        assert_eq!(
            proposal,
            Proposal {
                kind: TargetKind::Path,
                pattern: "file.txt".into(),
                access: Some(Access::Read),
            }
        );
    }

    #[test]
    fn path_proposal_inside_project_nested() {
        let path = PathRequest {
            absolute: "/project/src/main.rs".into(),
            relative: Some("src/main.rs".into()),
            home: None,
            access: Access::Write,
        };
        let proposal = proposal(&Request::Path(&path));
        assert_eq!(
            proposal,
            Proposal {
                kind: TargetKind::Path,
                pattern: "src/".into(),
                access: Some(Access::Write),
            }
        );
    }

    #[test]
    fn path_proposal_under_home_nested() {
        let path = PathRequest {
            absolute: "/home/u/.ssh/config".into(),
            relative: None,
            home: Some("/home/u".into()),
            access: Access::Read,
        };
        let proposal = proposal(&Request::Path(&path));
        assert_eq!(
            proposal,
            Proposal {
                kind: TargetKind::Path,
                pattern: "~/.ssh/".into(),
                access: Some(Access::Read),
            }
        );
    }

    #[test]
    fn path_proposal_under_home_file_directly() {
        let path = PathRequest {
            absolute: "/home/u/name".into(),
            relative: None,
            home: Some("/home/u".into()),
            access: Access::Read,
        };
        let proposal = proposal(&Request::Path(&path));
        assert_eq!(
            proposal,
            Proposal {
                kind: TargetKind::Path,
                pattern: "~/name".into(),
                access: Some(Access::Read),
            }
        );
    }

    #[test]
    fn path_proposal_absolute_nested() {
        let path = PathRequest {
            absolute: "/etc/passwd".into(),
            relative: None,
            home: None,
            access: Access::Read,
        };
        let proposal = proposal(&Request::Path(&path));
        assert_eq!(
            proposal,
            Proposal {
                kind: TargetKind::Path,
                pattern: "/etc/".into(),
                access: Some(Access::Read),
            }
        );
    }

    #[test]
    fn path_proposal_absolute_file_directly_under_root() {
        let path = PathRequest {
            absolute: "/etc".into(),
            relative: None,
            home: None,
            access: Access::Read,
        };
        let proposal = proposal(&Request::Path(&path));
        assert_eq!(
            proposal,
            Proposal {
                kind: TargetKind::Path,
                pattern: "/etc".into(),
                access: Some(Access::Read),
            }
        );
    }
}
