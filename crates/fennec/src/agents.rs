//! Agent discovery: embedded defaults, then `~/.agents/agents`, then
//! the project's `.agents/agents`, which overrides an agent of the same id.
//! Each agent carries where it was found (`AgentOrigin`), and the human's
//! CRUD writes the user ones.

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::agent::{AgentDefinition, AgentOrigin, parse_agent_md};

pub(crate) const DEFAULTS: [&str; 11] = [
    include_str!("../agents/plan.agent.md"),
    include_str!("../agents/builder.agent.md"),
    include_str!("../agents/verify.agent.md"),
    include_str!("../agents/review.agent.md"),
    include_str!("../agents/analyze.agent.md"),
    include_str!("../agents/ask.agent.md"),
    include_str!("../agents/narrate.agent.md"),
    include_str!("../agents/audit-architecture.agent.md"),
    include_str!("../agents/audit-security.agent.md"),
    include_str!("../agents/mockup.agent.md"),
    include_str!("../agents/remedy.agent.md"),
];

pub mod crud;

/// The agents a session in `workdir` can use.
pub fn discover(workdir: &Path) -> Vec<AgentDefinition> {
    discover_in(home().as_deref(), workdir)
}

pub(crate) fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// `~/.agents/agents`, where the human's own agents live.
pub(crate) fn user_dir(home: &Path) -> PathBuf {
    home.join(".agents").join("agents")
}

fn discover_in(home: Option<&Path>, workdir: &Path) -> Vec<AgentDefinition> {
    let mut agents = Vec::new();

    for content in DEFAULTS {
        let agent = parse_agent_md(content).expect("embedded default agent must parse");
        insert_or_override(&mut agents, agent);
    }

    if let Some(home) = home {
        for agent in load_agents_directory(&user_dir(home), AgentOrigin::User) {
            insert_or_override(&mut agents, agent);
        }
    }

    let dir = workdir.join(".agents").join("agents");
    for agent in load_agents_directory(&dir, AgentOrigin::Project) {
        insert_or_override(&mut agents, agent);
    }

    agents
}

/// The agents of one directory; missing dir → empty. A file that does not
/// parse is skipped here: discovery must never fail a session. The human sees
/// the reason through `read_user`, when they open the agent in the settings.
pub(crate) fn load_agents_directory(dir: &Path, origin: AgentOrigin) -> Vec<AgentDefinition> {
    let mut agents = Vec::new();
    for path in agent_files(dir) {
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        if let Ok(mut agent) = parse_agent_md(&content) {
            agent.origin = origin;
            agent.path = Some(path.to_string_lossy().into_owned());
            agents.push(agent);
        }
    }
    agents
}

/// The `*.agent.md` files of `dir`, in file-name order; empty when missing.
pub(crate) fn agent_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<_> = entries.filter_map(|entry| entry.ok()).collect();
    entries.sort_by_key(|entry| entry.file_name());

    entries
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".agent.md"))
        })
        .collect()
}

fn insert_or_override(agents: &mut Vec<AgentDefinition>, agent: AgentDefinition) {
    if let Some(slot) = agents.iter().position(|existing| existing.id == agent.id) {
        agents[slot] = agent;
    } else {
        agents.push(agent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::agent::CommandAccess;
    use crate::domain::id::new_id;

    fn fixture() -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("fennec-agents-{}", new_id()));
        let home = root.join("home");
        let workdir = root.join("workdir");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&workdir).unwrap();
        (root, home, workdir)
    }

    fn agents_dir(base: &Path) -> PathBuf {
        let dir = base.join(".agents").join("agents");
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_agent(agents: &Path, file_name: &str, content: &str) {
        std::fs::write(agents.join(file_name), content).unwrap();
    }

    #[test]
    fn defaults_load_plan_and_builder_with_metadata() {
        let (_root, _home, workdir) = fixture();
        let agents = discover_in(None, &workdir);

        assert_eq!(agents.len(), DEFAULTS.len());

        let plan = &agents[0];
        assert_eq!(plan.id, "plan");
        assert_eq!(plan.name, "Plan");
        assert_eq!(
            plan.description,
            "Explores the codebase and plans the task with the human, without changing anything"
        );
        assert!(!plan.subagent);
        assert_eq!(
            plan.allowed_tools,
            vec![
                "read_file",
                "run_command",
                "ask_user",
                "record_decision",
                "load_skill",
                "web_search",
                "web_fetch",
                "suggest_agent",
                "canvas"
            ]
        );
        assert_eq!(plan.commands, CommandAccess::ReadOnly);
        assert_eq!(plan.color.as_deref(), Some("#a855f7"));
        assert!(plan.prompt.contains("Plan Mode"));

        let builder = &agents[1];
        assert_eq!(builder.id, "builder");
        assert_eq!(builder.name, "Builder");
        assert_eq!(
            builder.description,
            "Implements the task by changing code and verifying it"
        );
        assert!(!builder.subagent);
        assert_eq!(
            builder.allowed_tools,
            vec![
                "read_file",
                "write_file",
                "edit_file",
                "run_command",
                "ask_user",
                "request_waiver",
                "propose_criteria",
                "record_decision",
                "load_skill",
                "web_search",
                "web_fetch",
                "suggest_agent",
                "canvas"
            ]
        );
        assert_eq!(builder.commands, CommandAccess::ReadWrite);
        assert_eq!(builder.color.as_deref(), Some("#3b82f6"));
        assert!(builder.prompt.contains("Build Mode"));
    }

    #[test]
    fn every_default_tool_is_a_known_tool() {
        let (_root, _home, workdir) = fixture();
        for agent in discover_in(None, &workdir) {
            for tool in &agent.allowed_tools {
                assert!(
                    crate::domain::agent::KNOWN_TOOLS.contains(&tool.as_str()),
                    "{}: {tool}",
                    agent.id
                );
            }
        }
    }

    #[test]
    fn read_only_analysis_defaults_are_not_subagents() {
        let (_root, _home, workdir) = fixture();
        let agents = discover_in(None, &workdir);
        for id in ["analyze", "ask", "narrate", "remedy"] {
            let agent = agents.iter().find(|agent| agent.id == id).unwrap();

            assert!(!agent.subagent);
            assert_eq!(agent.commands, CommandAccess::ReadOnly);
        }
    }

    #[test]
    fn the_framing_default_replaces_kickoff_and_keeps_its_tools() {
        let (_root, _home, workdir) = fixture();
        let agents = discover_in(None, &workdir);
        assert!(!agents.iter().any(|agent| agent.id == "kickoff"));

        let analyze = agents.iter().find(|agent| agent.id == "analyze").unwrap();
        // The merged agent must still be able to frame an empty project.
        for tool in ["ask_user", "record_decision", "propose_task"] {
            assert!(analyze.allows(tool), "{tool}");
        }
        assert_eq!(analyze.phase, Some(crate::domain::agent::Phase::Start));
    }

    #[test]
    fn audit_defaults_are_read_only_and_not_subagents() {
        let (_root, _home, workdir) = fixture();
        let agents = discover_in(None, &workdir);
        for id in ["audit-architecture", "audit-security"] {
            let audit_agent = agents.iter().find(|agent| agent.id == id).unwrap();

            assert!(!audit_agent.subagent, "{id}");
            assert_eq!(audit_agent.commands, CommandAccess::ReadOnly, "{id}");
        }
    }

    #[test]
    fn plan_allows_no_write_tool_and_no_run_command() {
        let (_root, _home, workdir) = fixture();
        let agents = discover_in(None, &workdir);
        let plan = agents.iter().find(|agent| agent.id == "plan").unwrap();

        assert!(!plan.allows("write_file"));
        assert!(!plan.allows("edit_file"));
        assert!(plan.allows("run_command"));
    }

    #[test]
    fn user_overrides_default_by_id_and_keeps_slot_order() {
        let (root, home, workdir) = fixture();
        let agents = agents_dir(&home);
        write_agent(
            &agents,
            "plan.agent.md",
            "---\nid: plan\nname: Custom Plan\ndescription: custom\n---\nCustom body\n",
        );

        let found = discover_in(Some(&home), &workdir);
        assert_eq!(found.len(), DEFAULTS.len());
        assert_eq!(found[0].id, "plan");
        assert_eq!(found[0].name, "Custom Plan");
        assert_eq!(found[0].prompt, "Custom body");
        assert_eq!(found[1].id, "builder");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn user_agent_added_alongside_defaults() {
        let (root, home, workdir) = fixture();
        let agents = agents_dir(&home);
        write_agent(
            &agents,
            "custom.agent.md",
            "---\nid: custom\nname: Custom\n---\nCustom body\n",
        );

        let found = discover_in(Some(&home), &workdir);
        assert_eq!(found.len(), DEFAULTS.len() + 1);
        assert!(found.iter().any(|agent| agent.id == "custom"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn project_beats_user_and_defaults() {
        let (root, home, workdir) = fixture();
        let home_agents = agents_dir(&home);
        write_agent(
            &home_agents,
            "plan.agent.md",
            "---\nid: plan\nname: User Plan\n---\nUser body\n",
        );

        let work_agents = agents_dir(&workdir);
        write_agent(
            &work_agents,
            "plan.agent.md",
            "---\nid: plan\nname: Project Plan\n---\nProject body\n",
        );

        let found = discover_in(Some(&home), &workdir);
        assert_eq!(found[0].id, "plan");
        assert_eq!(found[0].name, "Project Plan");
        assert_eq!(found[0].prompt, "Project body");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn origin_and_path_say_where_an_agent_was_found() {
        let (root, home, workdir) = fixture();
        write_agent(
            &agents_dir(&home),
            "plan.agent.md",
            "---\nid: plan\nname: User Plan\n---\nUser body\n",
        );
        write_agent(
            &agents_dir(&workdir),
            "plan.agent.md",
            "---\nid: plan\nname: Project Plan\n---\nProject body\n",
        );
        write_agent(
            &agents_dir(&home),
            "mine.agent.md",
            "---\nid: mine\n---\nMine\n",
        );

        let found = discover_in(Some(&home), &workdir);

        // The project overrides a default: the winner is marked `project`.
        let plan = found.iter().find(|agent| agent.id == "plan").unwrap();
        assert_eq!(plan.origin, AgentOrigin::Project);
        assert!(
            plan.path
                .as_deref()
                .unwrap()
                .starts_with(workdir.to_str().unwrap())
        );
        assert!(!plan.origin.editable());

        let mine = found.iter().find(|agent| agent.id == "mine").unwrap();
        assert_eq!(mine.origin, AgentOrigin::User);
        assert!(mine.origin.editable());

        let builder = found.iter().find(|agent| agent.id == "builder").unwrap();
        assert_eq!(builder.origin, AgentOrigin::Default);
        assert_eq!(builder.path, None);
        assert!(!builder.origin.editable());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_dirs_load_defaults_only() {
        let (root, home, workdir) = fixture();
        assert_eq!(discover_in(Some(&home), &workdir).len(), DEFAULTS.len());
        assert_eq!(discover_in(None, &workdir).len(), DEFAULTS.len());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn invalid_and_non_agent_files_are_skipped() {
        let (root, home, workdir) = fixture();
        let agents = agents_dir(&home);
        write_agent(&agents, "invalid.agent.md", "---\nname: no id\n---\nbody\n");
        std::fs::write(agents.join("notes.txt"), "not an agent").unwrap();

        let found = discover_in(Some(&home), &workdir);
        assert_eq!(found.len(), DEFAULTS.len());
        let _ = std::fs::remove_dir_all(&root);
    }
}
