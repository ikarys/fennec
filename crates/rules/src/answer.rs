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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decide::PathRequest;

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
