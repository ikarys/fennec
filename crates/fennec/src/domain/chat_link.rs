//! Chat-to-project links: a pure database reference used to group chats.

use crate::domain::project::{Project, SessionSummary};
use std::collections::BTreeMap;

/// Why a link is refused.
#[derive(Debug, PartialEq)]
pub enum LinkRefusal {
    NotAChat,
    ChatTarget,
}

/// A chat can be linked to any project but the chat's own scratch project;
/// unlinking (`None`) is always allowed for a chat.
pub fn check(
    session_project_id: &str,
    target_id: Option<&str>,
    chat_id: &str,
) -> Result<(), LinkRefusal> {
    if session_project_id != chat_id {
        return Err(LinkRefusal::NotAChat);
    }
    if target_id == Some(chat_id) {
        return Err(LinkRefusal::ChatTarget);
    }
    Ok(())
}

/// One group of the chats page.
#[derive(Debug, PartialEq)]
pub struct ChatGroup {
    pub project: Option<Project>,
    pub session_ids: Vec<String>,
}

/// Groups chats by linked project: linked groups by project name
/// (case-insensitive), then the unlinked group last. A link to a project not
/// in `projects` counts as unlinked. Sessions keep their input order inside a
/// group; empty groups are not returned.
pub fn group(sessions: &[SessionSummary], projects: &[Project]) -> Vec<ChatGroup> {
    // Keyed by (name, id): two projects with the same name stay two groups.
    let mut linked: BTreeMap<(String, String), (Option<Project>, Vec<String>)> = BTreeMap::new();
    let mut unlinked = Vec::new();
    for session in sessions {
        match session.linked_project_id.as_deref() {
            Some(target) => {
                let Some(project) = projects.iter().find(|p| p.id == target) else {
                    unlinked.push(session.id.clone());
                    continue;
                };
                linked
                    .entry((project.name.to_lowercase(), project.id.clone()))
                    .or_insert_with(|| (Some(project.clone()), Vec::new()))
                    .1
                    .push(session.id.clone());
            }
            None => unlinked.push(session.id.clone()),
        }
    }
    let mut out = linked
        .into_iter()
        .map(|(_, (project, session_ids))| ChatGroup {
            project,
            session_ids,
        })
        .collect::<Vec<_>>();
    if !unlinked.is_empty() {
        out.push(ChatGroup {
            project: None,
            session_ids: unlinked,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str, linked: Option<&str>) -> SessionSummary {
        SessionSummary {
            id: id.into(),
            project_id: "chat".into(),
            title: id.into(),
            created_at: 0,
            updated_at: 0,
            last_done: None,
            parent_id: None,
            awaiting_integration: false,
            linked_project_id: linked.map(str::to_string),
        }
    }

    fn project(id: &str, name: &str) -> Project {
        Project {
            id: id.into(),
            name: name.into(),
            workdir: "/w".into(),
        }
    }

    #[test]
    fn check_allows_chat_links_and_refuses_the_rest() {
        assert_eq!(check("chat", Some("p"), "chat"), Ok(()));
        assert_eq!(check("chat", None, "chat"), Ok(()));
        assert_eq!(check("p", Some("q"), "chat"), Err(LinkRefusal::NotAChat));
        assert_eq!(
            check("chat", Some("chat"), "chat"),
            Err(LinkRefusal::ChatTarget)
        );
    }

    #[test]
    fn group_orders_linked_groups_by_name_and_unlinked_last() {
        let sessions = [
            session("s1", Some("b")),
            session("s2", Some("a")),
            session("s3", Some("b")),
            session("s4", Some("missing")),
            session("s5", None),
        ];
        let projects = [project("a", "alpha"), project("b", "Beta")];

        let groups = group(&sessions, &projects);
        assert_eq!(groups.len(), 3);
        assert_eq!(
            groups[0].project.as_ref().map(|p| p.name.as_str()),
            Some("alpha")
        );
        assert_eq!(groups[0].session_ids, vec!["s2".to_string()]);
        assert_eq!(
            groups[1].project.as_ref().map(|p| p.name.as_str()),
            Some("Beta")
        );
        assert_eq!(
            groups[1].session_ids,
            vec!["s1".to_string(), "s3".to_string()]
        );
        assert!(groups[2].project.is_none());
        assert_eq!(
            groups[2].session_ids,
            vec!["s4".to_string(), "s5".to_string()]
        );
    }

    #[test]
    fn group_returns_no_empty_groups() {
        let projects = [project("a", "alpha")];
        assert!(group(&[], &projects).is_empty());

        let sessions = [session("s1", Some("missing"))];
        let groups = group(&sessions, &projects);
        assert_eq!(groups.len(), 1);
        assert!(groups[0].project.is_none());
        assert_eq!(groups[0].session_ids, vec!["s1".to_string()]);
    }

    #[test]
    fn group_keeps_two_projects_with_the_same_name_apart() {
        let sessions = [session("s1", Some("a")), session("s2", Some("b"))];
        let projects = [project("a", "web"), project("b", "web")];
        assert_eq!(group(&sessions, &projects).len(), 2);
    }
}
