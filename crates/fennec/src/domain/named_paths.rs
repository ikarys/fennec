//! Paths the human named in their own words: a read outside the project the human
//! pointed at needs no second approval. The matching is deliberately narrow: only
//! absolute POSIX-style paths with enough specificity can grant it.

use crate::domain::event::{Event, Role};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

/// The texts the human wrote in this session: their messages (not system-generated)
/// and their answers to ask_user (the tool result of a `chat.ask_user` call id).
pub fn human_texts(events: &[Event]) -> Vec<&str> {
    let ask_call_ids: HashSet<&str> = events
        .iter()
        .filter_map(|event| match event {
            Event::ChatAskUser { call_id, .. } => Some(call_id.as_str()),
            _ => None,
        })
        .collect();

    events
        .iter()
        .filter_map(|event| match event {
            Event::MessageStart {
                role,
                content,
                is_system_generated,
                ..
            } if role == &Role::User && !is_system_generated => content.as_deref(),
            Event::ToolResult {
                tool_call_id,
                result,
                ..
            } if ask_call_ids.contains(tool_call_id.as_str()) => result.output.as_deref(),
            _ => None,
        })
        .collect()
}

/// Whether `subject` (an absolute path) is, or lies under, an absolute path the human
/// wrote in one of `texts`.
pub fn named_by_human(subject: &str, texts: &[&str], home: Option<&Path>) -> bool {
    let subject_path = Path::new(subject);
    if has_parent_component(subject_path) {
        return false;
    }

    texts.iter().any(|text| {
        named_path_tokens(text, home).into_iter().any(|token| {
            !has_parent_component(&token)
                && is_posix_absolute(token.to_str().unwrap_or_default())
                && normal_component_count(&token) >= 2
                && subject_path.starts_with(&token)
        })
    })
}

fn named_path_tokens(text: &str, home: Option<&Path>) -> Vec<PathBuf> {
    let delimited = text.replace(['`', '"', '\'', '(', ')', '<', '>', '[', ']'], " ");

    delimited
        .split_ascii_whitespace()
        .map(|token| token.trim_end_matches(['.', ',', ':', ';', '!', '?']))
        .filter(|token| !token.is_empty())
        .filter_map(|token| normalize_token(token, home))
        .collect()
}

fn normalize_token(token: &str, home: Option<&Path>) -> Option<PathBuf> {
    let normalized = token.replace('\\', "/");

    let absolute = if let Some(wsl_path) = strip_wsl_unc_prefix(&normalized) {
        wsl_path
    } else if let Some(rest) = normalized.strip_prefix("~/") {
        home?.join(rest).to_string_lossy().into_owned()
    } else {
        normalized
    };

    Some(PathBuf::from(absolute))
}

fn strip_wsl_unc_prefix(path: &str) -> Option<String> {
    let lower = path.to_ascii_lowercase();
    let prefix_len = if lower.starts_with("//wsl.localhost/") {
        "//wsl.localhost/".len()
    } else if lower.starts_with("//wsl$/") {
        "//wsl$/".len()
    } else {
        return None;
    };

    let after_prefix = &path[prefix_len..];
    let (_, rest) = after_prefix.split_once('/')?;
    Some(format!("/{rest}"))
}

fn is_posix_absolute(path: &str) -> bool {
    path.starts_with('/')
}

fn normal_component_count(path: &Path) -> usize {
    path.components()
        .filter(|component| !matches!(component, Component::RootDir))
        .count()
}

fn has_parent_component(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

#[cfg(test)]
mod tests {
    use super::{human_texts, named_by_human};
    use crate::domain::event::{Event, Role, ToolResult};
    use std::path::Path;

    fn message(role: Role, content: &str, is_system_generated: bool) -> Event {
        Event::MessageStart {
            message_id: "m".into(),
            role,
            content: Some(content.into()),
            is_system_generated,
            metadata: Default::default(),
            attachments: Default::default(),
        }
    }

    fn ask_user(call_id: &str) -> Event {
        Event::ChatAskUser {
            call_id: call_id.into(),
            question: String::new(),
            ask_type: Default::default(),
            options: Default::default(),
        }
    }

    fn tool_result(tool_call_id: &str, output: &str) -> Event {
        Event::ToolResult {
            message_id: "m".into(),
            tool_call_id: tool_call_id.into(),
            result: ToolResult::success(output.into(), false),
        }
    }

    #[test]
    fn exact_file_is_named() {
        assert!(named_by_human(
            "/home/user/file.txt",
            &["Read /home/user/file.txt"],
            None
        ));
    }

    #[test]
    fn parent_folder_is_named() {
        assert!(named_by_human(
            "/home/user/docs/note.md",
            &["Read /home/user/docs"],
            None
        ));
    }

    #[test]
    fn trailing_slash_and_punctuation_are_ignored() {
        assert!(named_by_human(
            "/home/user/file.txt",
            &["Read /home/user/file.txt,"],
            None
        ));
        assert!(named_by_human(
            "/home/user/file.txt",
            &["Read /home/user/"],
            None
        ));
    }

    #[test]
    fn backticks_and_quotes_are_ignored() {
        assert!(named_by_human(
            "/home/user/file.txt",
            &["Read `/home/user/file.txt`"],
            None
        ));
        assert!(named_by_human(
            "/home/user/file.txt",
            &["Read \"/home/user/file.txt\""],
            None
        ));
    }

    #[test]
    fn wsl_unc_path_is_named() {
        assert!(named_by_human(
            "/home/t/x",
            &[r"Read \\wsl.localhost\Ubuntu\home\t\x"],
            None
        ));
    }

    #[test]
    fn home_prefix_is_expanded() {
        let home = Path::new("/home/user");
        assert!(named_by_human(
            "/home/user/notes/a.md",
            &["Read ~/notes/a.md"],
            Some(home)
        ));
        assert!(!named_by_human(
            "/home/user/notes/a.md",
            &["Read ~/notes/a.md"],
            None
        ));
    }

    #[test]
    fn sibling_prefix_is_not_named() {
        assert!(!named_by_human("/a/bc", &["Read /a/b"], None));
    }

    #[test]
    fn one_component_path_is_too_broad() {
        assert!(!named_by_human("/home/user", &["Read /home"], None));
    }

    #[test]
    fn parent_directory_components_are_not_matched() {
        assert!(!named_by_human("/b", &["Read /a/../b"], None));
        assert!(!named_by_human("/a/../b", &["Read /b"], None));
    }

    #[test]
    fn human_texts_keeps_user_and_ask_answers_only() {
        let events = vec![
            message(Role::User, "Read /home/user/named.txt", false),
            message(Role::Assistant, "Read /home/user/assistant.txt", false),
            message(Role::User, "Read /home/user/system.txt", true),
            ask_user("ask1"),
            tool_result("other", "Read /home/user/other.txt"),
            tool_result("ask1", "Read /home/user/answer.txt"),
        ];

        assert_eq!(
            human_texts(&events),
            vec!["Read /home/user/named.txt", "Read /home/user/answer.txt"]
        );
    }
}
