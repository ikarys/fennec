//! English commit subject: the subject the model proposes for an integration,
//! built from the session's request, title, branch commits and changed files.

use super::event::Event;

/// The prompt template; `prompt` fills its placeholders.
pub const COMMIT_SUBJECT_PROMPT: &str =
    "Write ONE Conventional Commits subject line in English for the change
described below, whatever the language of the inputs.
Format: <type>: <summary>
Type must be one of: feat, fix, docs, refactor, test, chore, perf, build, ci, style.
Use imperative mood, lowercase after the colon, no trailing period, at most 72
characters.
Return ONLY the line.
Describe the commits and files below, not the session as a whole.

The human's requests since the last integration:
{request}

Session title:
{title}

Commits on the branch:
{commits}

Files changed:
{files}";

const COMMIT_TYPES: &[&str] = &[
    "feat", "fix", "docs", "refactor", "test", "chore", "perf", "build", "ci", "style",
];

/// The model-facing prompt for one integration commit subject.
pub fn prompt(request: &str, title: &str, commits: &[&str], files: &[(&str, u64, u64)]) -> String {
    let request: String = request.chars().take(2000).collect();
    let commits = commit_lines(commits);
    let files = file_lines(files);

    COMMIT_SUBJECT_PROMPT
        .replace("{request}", &request)
        .replace("{title}", title)
        .replace("{commits}", &commits)
        .replace("{files}", &files)
}

/// The human's requests since the last integration; a session integrated twice
/// must not be described by its first request again.
pub fn request_since_integration(events: &[Event]) -> String {
    let start = match events
        .iter()
        .rposition(|event| matches!(event, Event::IntegrationApplied { .. }))
    {
        Some(index) => index + 1,
        None => 0,
    };

    let requests: Vec<&str> = events
        .iter()
        .skip(start)
        .filter_map(crate::domain::conversation::human_request)
        .collect();

    if requests.is_empty() {
        crate::domain::conversation::first_request(events).unwrap_or_default()
    } else {
        requests.join("\n\n")
    }
}

/// Cleans the model's answer into a valid English Conventional Commit subject.
pub fn clean(raw: &str) -> Option<String> {
    let line = raw.lines().map(str::trim).find(|line| !line.is_empty())?;
    let line = strip_surrounding(line);
    let line = line.strip_suffix('.').unwrap_or(line).trim();

    let (head, summary) = line.split_once(':')?;
    let head = head.trim();
    let summary = summary.trim();
    if summary.is_empty() {
        return None;
    }

    let cut = head.find(['(', '!']).unwrap_or(head.len());
    let (type_part, rest) = head.split_at(cut);
    let commit_type = type_part.trim().to_lowercase();
    if !COMMIT_TYPES.contains(&commit_type.as_str()) {
        return None;
    }

    let subject = format!("{commit_type}{rest}: {summary}");
    if subject.chars().count() > 72 {
        return None;
    }
    Some(subject)
}

fn commit_lines(commits: &[&str]) -> String {
    let lines: Vec<String> = commits
        .iter()
        .copied()
        .filter(|subject| !subject.starts_with("fennec: agent work"))
        .map(|subject| format!("- {subject}"))
        .collect();

    if lines.is_empty() {
        "(none)".to_string()
    } else {
        lines.join("\n")
    }
}

fn file_lines(files: &[(&str, u64, u64)]) -> String {
    let mut lines: Vec<String> = files
        .iter()
        .take(60)
        .map(|(path, added, removed)| format!("- {path} (+{added} -{removed})"))
        .collect();

    if files.len() > 60 {
        lines.push(format!("- … and {} more", files.len() - 60));
    }

    if lines.is_empty() {
        "(none)".to_string()
    } else {
        lines.join("\n")
    }
}

fn strip_surrounding(subject: &str) -> &str {
    if subject.len() < 2 {
        return subject;
    }

    for quote in ['"', '\'', '`'] {
        if let Some(inner) = subject
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }

    subject
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::Role;

    #[test]
    fn prompt_includes_the_request() {
        let p = prompt("add the feature", "Title", &[], &[]);
        assert!(p.contains("The human's requests since the last integration:\nadd the feature"));
    }

    #[test]
    fn prompt_skips_fennec_auto_commits_and_writes_none() {
        let p = prompt("r", "t", &["fennec: agent work in session x"], &[]);
        assert!(p.contains("Commits on the branch:\n(none)"));
    }

    #[test]
    fn prompt_caps_files() {
        let files = vec![("a.rs", 1, 0); 61];
        let p = prompt("r", "t", &[], &files);
        assert_eq!(p.matches("- a.rs (+1 -0)").count(), 60);
        assert!(p.contains("- … and 1 more"));
    }

    #[test]
    fn clean_accepts_valid_subjects() {
        assert_eq!(clean("feat: add x"), Some("feat: add x".into()));
        assert_eq!(clean("\"fix(ui): y.\""), Some("fix(ui): y".into()));
        assert_eq!(clean("Feat: z"), Some("feat: z".into()));
        assert_eq!(clean("feat!: drop api"), Some("feat!: drop api".into()));
    }

    #[test]
    fn clean_rejects_invalid_subjects() {
        assert_eq!(clean("afficher les boutons"), None);
        assert_eq!(clean("wip: x"), None);
        assert_eq!(clean("feat:   "), None);
        let long = "feat: ".to_string() + &"x".repeat(84);
        assert_eq!(clean(&long), None);
        assert_eq!(clean(""), None);
    }

    fn user_request(id: &str, content: &str) -> Event {
        Event::MessageStart {
            message_id: id.into(),
            role: Role::User,
            content: Some(content.into()),
            is_system_generated: false,
            metadata: None,
            attachments: Vec::new(),
        }
    }

    fn integration_applied() -> Event {
        Event::IntegrationApplied {
            branch: "branch".into(),
            base: "base".into(),
            commits: Vec::new(),
            head: "head".into(),
            message: String::new(),
            acknowledged: Vec::new(),
            protected_tests: Vec::new(),
        }
    }

    #[test]
    fn request_since_integration_returns_only_requests_after_last_integration() {
        let events = vec![
            user_request("1", "first"),
            integration_applied(),
            user_request("2", "second"),
        ];
        assert_eq!(request_since_integration(&events), "second");
    }

    #[test]
    fn request_since_integration_joins_all_requests_without_integration() {
        let events = vec![user_request("1", "first"), user_request("2", "second")];
        assert_eq!(request_since_integration(&events), "first\n\nsecond");
    }

    #[test]
    fn request_since_integration_falls_back_to_first_request() {
        let events = vec![user_request("1", "first"), integration_applied()];
        assert_eq!(request_since_integration(&events), "first");
    }
}
