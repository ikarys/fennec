//! What the project file explorer accepts from a request, decided without I/O:
//! a path relative to the project, a session whose branch is read, a commit message.

pub const MAX_FILE_BYTES: u64 = 1024 * 1024;
pub const MAX_LISTED: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelPath(String);

impl RelPath {
    pub fn parse(raw: &str) -> Result<RelPath, String> {
        if raw.is_empty() {
            return Err("path is empty".into());
        }
        if raw.len() > 4096 {
            return Err("path is too long".into());
        }
        if raw.contains('\0') {
            return Err("path contains a NUL byte".into());
        }
        if raw.contains('\\') {
            return Err("path contains a backslash".into());
        }
        if raw.starts_with('/') {
            return Err("path must be relative".into());
        }
        if raw.split('/').any(|component| {
            component.is_empty()
                || component == "."
                || component == ".."
                || component.eq_ignore_ascii_case(".git")
        }) {
            return Err("path has an invalid component".into());
        }
        Ok(RelPath(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBranch(String);

impl SessionBranch {
    pub fn parse(session_id: &str) -> Result<SessionBranch, String> {
        if session_id.is_empty() || session_id.len() > 128 {
            return Err("session id must be 1 to 128 characters".into());
        }
        if session_id.starts_with('-') {
            return Err("session id cannot start with '-'".into());
        }
        if !session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("session id may use ASCII letters, digits, and '-'".into());
        }
        Ok(SessionBranch(format!("fennec/{session_id}")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitMessage(String);

impl CommitMessage {
    pub fn parse(raw: &str) -> Result<CommitMessage, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("commit message is empty".into());
        }
        if trimmed.contains('\0') {
            return Err("commit message contains a NUL byte".into());
        }
        if trimmed.chars().count() > 2000 {
            return Err("commit message is too long".into());
        }
        Ok(CommitMessage(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_nested_path_is_accepted() {
        let path = RelPath::parse("src/main.rs").unwrap();
        assert_eq!(path.as_str(), "src/main.rs");
    }

    #[test]
    fn traversal_absolute_and_git_paths_are_refused() {
        for raw in [
            "",
            "../x",
            "a/../../x",
            "/x",
            ".git/config",
            "sub/.GIT/hooks/x",
            "a//b",
            "a/",
            "a\0b",
            "a\\b",
            ".",
            "..",
        ] {
            assert!(RelPath::parse(raw).is_err(), "{raw} should be refused");
        }
    }

    #[test]
    fn session_ids_with_slashes_dots_or_a_leading_dash_are_refused() {
        let too_long = "a".repeat(129);
        for id in ["", "-x", "../main", "a/b", "a..b", too_long.as_str()] {
            assert!(SessionBranch::parse(id).is_err(), "{id} should be refused");
        }
        let branch = SessionBranch::parse("18e6709f8abdf916-10696a-24").unwrap();
        assert_eq!(branch.as_str(), "fennec/18e6709f8abdf916-10696a-24");
    }

    #[test]
    fn a_blank_message_is_refused_and_a_padded_one_is_trimmed() {
        assert!(CommitMessage::parse("   ").is_err());
        let message = CommitMessage::parse("  save file \n").unwrap();
        assert_eq!(message.as_str(), "save file");
    }
}
