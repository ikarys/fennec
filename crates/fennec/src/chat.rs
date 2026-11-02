//! The chat: a session with no project, folder or git. It runs in a scratch
//! directory under the store, so the agent's tools have a workdir and the
//! model can talk without a repository to act on.

use crate::domain::project::Project;
use std::path::Path;

/// The chat project's id: stable, so the UI can reference it by this string.
pub const ID: &str = "chat";

/// The chat project's name, as listed to the human.
pub const NAME: &str = "Chat";

/// The chat's scratch workdir: `chat/` next to the store's database.
pub fn workdir(db_path: &Path) -> std::path::PathBuf {
    db_path.parent().unwrap_or(Path::new(".")).join("chat")
}

/// Whether a project is the chat's scratch project.
pub fn is_chat(project: &Project) -> bool {
    project.id == ID
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::project::Project;

    fn project(id: &str) -> Project {
        Project {
            id: id.into(),
            name: "n".into(),
            workdir: "/w".into(),
        }
    }

    #[test]
    fn chat_workdir_is_next_to_the_store() {
        let dir = workdir(Path::new("/home/u/.local/share/fennec/fennec.db"));
        assert_eq!(
            dir,
            std::path::PathBuf::from("/home/u/.local/share/fennec/chat")
        );
    }

    #[test]
    fn only_the_chat_project_is_marked_chat() {
        assert!(is_chat(&project(ID)));
        assert!(!is_chat(&project("other")));
    }
}
