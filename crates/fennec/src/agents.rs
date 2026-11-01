//! Port of openfox `agents/registry.ts` discovery (`loadDefaultAgents`,
//! `loadAllAgents`, `loadProjectAgents`): embedded defaults, then
//! `~/.agents/agents`, then the project's `.agents/agents`, which overrides
//! an agent of the same id.
//!
//! Not ported: openfox's own config dir and `.openfox/agents`, CRUD
//! (save/delete), overrides listing, per-agent model overrides, project/DB
//! default agent setting, granular `tool:action` permissions and MCP rules,
//! `results`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::agent::{AgentDefinition, parse_agent_md};

const DEFAULTS: [&str; 11] = [
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
    include_str!("../agents/kickoff.agent.md"),
];

/// openfox `loadAllAgents`: the agents a session in `workdir` can use.
pub fn discover(workdir: &Path) -> Vec<AgentDefinition> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    discover_in(home.as_deref(), workdir)
}

fn discover_in(home: Option<&Path>, workdir: &Path) -> Vec<AgentDefinition> {
    let mut agents = Vec::new();

    for content in DEFAULTS {
        let agent = parse_agent_md(content).expect("embedded default agent must parse");
        insert_or_override(&mut agents, agent);
    }

    if let Some(home) = home {
        let dir = home.join(".agents").join("agents");
        for agent in load_agents_directory(&dir) {
            insert_or_override(&mut agents, agent);
        }
    }

    let dir = workdir.join(".agents").join("agents");
    for agent in load_agents_directory(&dir) {
        insert_or_override(&mut agents, agent);
    }

    agents
}

/// openfox `loadAgentsFromDir`; missing dir → empty.
fn load_agents_directory(dir: &Path) -> Vec<AgentDefinition> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut entries: Vec<_> = entries.filter_map(|entry| entry.ok()).collect();
    entries.sort_by_key(|entry| entry.file_name());

    let mut agents = Vec::new();
    for entry in entries {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if !path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().ends_with(".agent.md"))
        {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        if let Some(agent) = parse_agent_md(&content) {
            agents.push(agent);
        }
    }

    agents
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
                "suggest_agent"
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
                "propose_criteria",
                "record_decision",
                "load_skill",
                "web_search",
                "web_fetch",
                "suggest_agent"
            ]
        );
        assert_eq!(builder.commands, CommandAccess::ReadWrite);
        assert_eq!(builder.color.as_deref(), Some("#3b82f6"));
        assert!(builder.prompt.contains("Build Mode"));
    }

    #[test]
    fn read_only_analysis_defaults_are_not_subagents() {
        let (_root, _home, workdir) = fixture();
        let agents = discover_in(None, &workdir);
        for id in ["kickoff", "analyze", "ask", "narrate"] {
            let agent = agents.iter().find(|agent| agent.id == id).unwrap();

            assert!(!agent.subagent);
            assert_eq!(agent.commands, CommandAccess::ReadOnly);
        }
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
