//! Workflow validation: the issues the editor shows next to a node or edge.

use super::definition::{AgentContext, BRANCH, ConditionTest, EXHAUSTED, Edge, NodeKind, Workflow};
use super::template::{check_name, names};
use crate::domain::agent::AgentDefinition;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge: Option<usize>,
    pub message: String,
}

pub fn validate(workflow: &Workflow, agents: &[AgentDefinition]) -> Vec<Issue> {
    let mut issues = Vec::new();

    check_basic(workflow, &mut issues);
    let basic_ok = issues.is_empty();

    check_node_fields(workflow, agents, &mut issues);
    check_edges(workflow, &mut issues);
    check_templates(workflow, &mut issues);
    check_wiring(workflow, &mut issues);

    if basic_ok {
        check_reachability(workflow, &mut issues);
        check_parallel(workflow, agents, &mut issues);
        check_uncapped_loops(workflow, &mut issues);
    }

    issues
}

fn issue(node_id: Option<String>, edge: Option<usize>, message: String) -> Issue {
    Issue {
        node_id,
        edge,
        message,
    }
}

fn valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => return false,
    }

    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn check_basic(workflow: &Workflow, issues: &mut Vec<Issue>) {
    if !valid_id(&workflow.id) {
        issues.push(issue(
            None,
            None,
            "workflow id must match ^[a-z0-9][a-z0-9-]*$".into(),
        ));
    }

    let mut seen = HashSet::new();
    let mut starts = 0;
    let mut ends = 0;

    for node in &workflow.nodes {
        if node.id.is_empty() {
            issues.push(issue(
                Some(node.id.clone()),
                None,
                "node id is empty".into(),
            ));
        } else if !valid_id(&node.id) {
            issues.push(issue(
                Some(node.id.clone()),
                None,
                "node id must match ^[a-z0-9][a-z0-9-]*$".into(),
            ));
        }

        if !seen.insert(node.id.clone()) {
            issues.push(issue(
                Some(node.id.clone()),
                None,
                format!("duplicate node id {}", node.id),
            ));
        }

        if node.kind.is_end() {
            ends += 1;
        }
        if matches!(node.kind, NodeKind::Start) {
            starts += 1;
        }
    }

    if starts == 0 {
        issues.push(issue(None, None, "missing start node".into()));
    }
    if starts > 1 {
        issues.push(issue(None, None, "more than one start node".into()));
    }
    if ends == 0 {
        issues.push(issue(None, None, "missing end node".into()));
    }
}

fn check_node_fields(workflow: &Workflow, agents: &[AgentDefinition], issues: &mut Vec<Issue>) {
    for node in &workflow.nodes {
        match &node.kind {
            NodeKind::Agent {
                agent,
                outputs,
                context,
                ..
            } => {
                match agents.iter().find(|definition| &definition.id == agent) {
                    None => issues.push(issue(
                        Some(node.id.clone()),
                        None,
                        format!("unknown agent {agent}"),
                    )),
                    // A sub-agent is never a mode of the human's session.
                    Some(definition)
                        if definition.subagent && *context == AgentContext::Session =>
                    {
                        issues.push(issue(
                            Some(node.id.clone()),
                            None,
                            format!("sub-agent {agent} runs only in a fresh context"),
                        ))
                    }
                    Some(_) => {}
                }

                if outputs.is_empty() {
                    issues.push(issue(
                        Some(node.id.clone()),
                        None,
                        "agent outputs must not be empty".into(),
                    ));
                }

                let mut seen = HashSet::new();
                for port in outputs {
                    if !seen.insert(port.clone()) {
                        issues.push(issue(
                            Some(node.id.clone()),
                            None,
                            format!("duplicate agent output {port}"),
                        ));
                    }
                }
            }
            NodeKind::Human { choices, .. } => {
                if choices.is_empty() {
                    issues.push(issue(
                        Some(node.id.clone()),
                        None,
                        "human choices must not be empty".into(),
                    ));
                }

                let mut seen = HashSet::new();
                for choice in choices {
                    if !seen.insert(choice.clone()) {
                        issues.push(issue(
                            Some(node.id.clone()),
                            None,
                            format!("duplicate human choice {choice}"),
                        ));
                    }
                }
            }
            NodeKind::Shell { command, .. } => {
                if command.trim().is_empty() {
                    issues.push(issue(
                        Some(node.id.clone()),
                        None,
                        "shell command is empty".into(),
                    ));
                }
            }
            NodeKind::Criteria { plan } => {
                let plan_is_agent = workflow
                    .node(plan)
                    .is_some_and(|plan| matches!(plan.kind, NodeKind::Agent { .. }));
                if !plan_is_agent {
                    issues.push(issue(
                        Some(node.id.clone()),
                        None,
                        format!("criteria plan {plan} is not an agent node"),
                    ));
                }
            }
            NodeKind::Condition { test, against, .. }
                if *test != ConditionTest::Empty && against.is_none() =>
            {
                issues.push(issue(
                    Some(node.id.clone()),
                    None,
                    "condition needs an against value".into(),
                ));
            }
            _ => {}
        }
    }
}

fn check_edges(workflow: &Workflow, issues: &mut Vec<Issue>) {
    let mut seen_pairs = HashSet::new();

    for (index, edge) in workflow.edges.iter().enumerate() {
        let from_node = workflow.node(&edge.from);
        let to_node = workflow.node(&edge.to);

        if from_node.is_none() {
            issues.push(issue(
                None,
                Some(index),
                format!("edge from unknown node {}", edge.from),
            ));
        }
        if to_node.is_none() {
            issues.push(issue(
                None,
                Some(index),
                format!("edge to unknown node {}", edge.to),
            ));
        }

        if let Some(from_node) = from_node {
            if from_node.kind.is_end() {
                issues.push(issue(
                    Some(edge.from.clone()),
                    Some(index),
                    format!("edge leaves end node {}", edge.from),
                ));
            }

            if !workflow.ports(from_node).contains(&edge.port) {
                issues.push(issue(
                    Some(edge.from.clone()),
                    Some(index),
                    format!("port {} is not an output of {}", edge.port, edge.from),
                ));
            }
        }

        if let Some(to_node) = to_node
            && matches!(to_node.kind, NodeKind::Start)
        {
            issues.push(issue(
                Some(edge.to.clone()),
                Some(index),
                format!("edge enters start node {}", edge.to),
            ));
        }

        let parallel_branch =
            from_node.is_some_and(|node| node.kind.is_parallel()) && edge.port == BRANCH;
        if !parallel_branch {
            let pair = (edge.from.clone(), edge.port.clone());
            if !seen_pairs.insert(pair) {
                issues.push(issue(
                    Some(edge.from.clone()),
                    Some(index),
                    format!("port {} of {} is wired twice", edge.port, edge.from),
                ));
            }
        }

        if edge.max_passes.is_some_and(|max| max == 0) {
            issues.push(issue(
                None,
                Some(index),
                "maxPasses must be at least 1".into(),
            ));
        }
    }
}

fn check_templates(workflow: &Workflow, issues: &mut Vec<Issue>) {
    for node in &workflow.nodes {
        let texts: Vec<&str> = match &node.kind {
            NodeKind::Agent {
                prompt,
                retry_prompt,
                ..
            } => {
                let mut texts = vec![prompt.as_str()];
                if let Some(retry) = retry_prompt {
                    texts.push(retry.as_str());
                }
                texts
            }
            NodeKind::Shell { command, .. } => vec![command.as_str()],
            NodeKind::Human { question, .. } => vec![question.as_str()],
            NodeKind::Condition { value, .. } => vec![value.as_str()],
            _ => Vec::new(),
        };

        for text in texts {
            for name in names(text) {
                if let Err(message) = check_name(workflow, &name) {
                    issues.push(issue(Some(node.id.clone()), None, message));
                }
            }
        }
    }
}

fn check_wiring(workflow: &Workflow, issues: &mut Vec<Issue>) {
    for node in &workflow.nodes {
        if node.kind.is_end() {
            continue;
        }

        for port in workflow.ports(node) {
            if port == EXHAUSTED {
                continue;
            }

            let wired = workflow
                .edges
                .iter()
                .any(|edge| edge.from == node.id && edge.port == port);
            if !wired {
                issues.push(issue(
                    Some(node.id.clone()),
                    None,
                    format!("port {} of {} is not wired", port, node.id),
                ));
            }
        }
    }
}

fn check_reachability(workflow: &Workflow, issues: &mut Vec<Issue>) {
    let mut reachable = HashSet::new();
    if let Some(start) = workflow.start() {
        let mut queue = VecDeque::new();
        queue.push_back(start.id.clone());
        reachable.insert(start.id.clone());

        while let Some(id) = queue.pop_front() {
            for edge in workflow.edges.iter().filter(|edge| edge.from == id) {
                if reachable.insert(edge.to.clone()) {
                    queue.push_back(edge.to.clone());
                }
            }
        }
    }

    for node in &workflow.nodes {
        if !reachable.contains(&node.id) {
            issues.push(issue(
                Some(node.id.clone()),
                None,
                format!("node {} is not reachable from start", node.id),
            ));
        }
    }

    let mut can_reach_end = HashSet::new();
    let mut queue = workflow
        .nodes
        .iter()
        .filter(|node| node.kind.is_end())
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();

    for id in &queue {
        can_reach_end.insert(id.clone());
    }

    while let Some(id) = queue.pop() {
        for edge in workflow.edges.iter().filter(|edge| edge.to == id) {
            if can_reach_end.insert(edge.from.clone()) {
                queue.push(edge.from.clone());
            }
        }
    }

    for node in &workflow.nodes {
        if !can_reach_end.contains(&node.id) {
            issues.push(issue(
                Some(node.id.clone()),
                None,
                format!("node {} cannot reach an end", node.id),
            ));
        }
    }
}

fn check_parallel(workflow: &Workflow, agents: &[AgentDefinition], issues: &mut Vec<Issue>) {
    for node in workflow.nodes.iter().filter(|node| node.kind.is_parallel()) {
        let parallel_id = &node.id;
        let entries: Vec<&Edge> = workflow.edges_from(parallel_id, BRANCH).collect();
        if entries.len() < 2 {
            issues.push(issue(
                Some(parallel_id.clone()),
                None,
                format!("parallel {parallel_id} needs at least two branch edges"),
            ));
        }
        let Some(join) = workflow.join_of(parallel_id) else {
            issues.push(issue(
                Some(parallel_id.clone()),
                None,
                format!("parallel {parallel_id} has no join"),
            ));
            continue;
        };

        let mut claimed = HashSet::new();
        let mut writing_branches = 0;
        for entry in entries {
            let branch = branch_nodes(workflow, &entry.to, &join);
            check_branch(workflow, parallel_id, &entry.to, &join, &branch, issues);
            for id in &branch {
                if !claimed.insert(id.clone()) {
                    issues.push(issue(
                        Some(id.clone()),
                        None,
                        format!("branches of {parallel_id} overlap at {id}"),
                    ));
                }
            }
            if branch_writes(workflow, &branch, agents) {
                writing_branches += 1;
            }
        }
        if writing_branches > 1 {
            issues.push(issue(
                Some(parallel_id.clone()),
                None,
                format!("only one branch of {parallel_id} may write"),
            ));
        }
    }
    check_joins(workflow, issues);
}

/// One branch: reaches the join, holds no node that cannot run in a branch,
/// never pauses, and is entered only from its parallel node.
fn check_branch(
    workflow: &Workflow,
    parallel_id: &str,
    entry: &str,
    join: &str,
    branch: &[String],
    issues: &mut Vec<Issue>,
) {
    let reaches_join = entry == join
        || workflow
            .edges
            .iter()
            .any(|edge| edge.to == join && branch.contains(&edge.from));
    if !reaches_join {
        issues.push(issue(
            Some(parallel_id.to_string()),
            None,
            format!("branch of {parallel_id} from {entry} does not reach the join"),
        ));
    }

    for id in branch {
        let forbidden = workflow.node(id).is_some_and(|node| {
            node.kind.is_end() || node.kind.is_parallel() || node.kind.is_human()
        });
        if forbidden || id == parallel_id {
            issues.push(issue(
                Some(id.clone()),
                None,
                format!("branch of {parallel_id} contains a forbidden node {id}"),
            ));
        }
        // Criteria events go to the session log, which only the main walk writes.
        match workflow.node(id).map(|node| &node.kind) {
            Some(NodeKind::Criteria { .. }) => issues.push(issue(
                Some(id.clone()),
                None,
                "a criteria node cannot run in a parallel branch".into(),
            )),
            Some(NodeKind::Agent { verdicts: true, .. }) => issues.push(issue(
                Some(id.clone()),
                None,
                "verdicts are recorded by the main walk only".into(),
            )),
            _ => {}
        }
    }

    for (index, edge) in workflow.edges.iter().enumerate() {
        let from_inside = branch.contains(&edge.from);
        if edge.pause && from_inside && (branch.contains(&edge.to) || edge.to == join) {
            issues.push(issue(
                Some(edge.from.clone()),
                Some(index),
                format!("branch of {parallel_id} has a paused edge"),
            ));
        }
        if branch.contains(&edge.to) && edge.from != parallel_id && !from_inside {
            issues.push(issue(
                Some(edge.to.clone()),
                Some(index),
                format!("branch of {parallel_id} is entered from {}", edge.from),
            ));
        }
    }
}

/// Every join closes exactly one parallel node.
fn check_joins(workflow: &Workflow, issues: &mut Vec<Issue>) {
    for node in workflow.nodes.iter().filter(|node| node.kind.is_join()) {
        let join_id = &node.id;
        let users = workflow
            .nodes
            .iter()
            .filter(|candidate| {
                candidate.kind.is_parallel()
                    && workflow.join_of(&candidate.id).as_ref() == Some(join_id)
            })
            .count();
        let problem = match users {
            0 => "has no parallel",
            1 => continue,
            _ => "is used by more than one parallel",
        };
        issues.push(issue(
            Some(join_id.clone()),
            None,
            format!("join {join_id} {problem}"),
        ));
    }
}

fn check_uncapped_loops(workflow: &Workflow, issues: &mut Vec<Issue>) {
    if let Some(node_id) = find_uncapped_loop(workflow) {
        issues.push(issue(
            Some(node_id),
            None,
            "this loop has no cap: set maxPasses on one of its edges, pause one, \
             or put a human node on it"
                .into(),
        ));
    }
}

fn branch_nodes(workflow: &Workflow, entry: &str, join: &str) -> Vec<String> {
    let mut nodes = Vec::new();
    let mut seen = HashSet::new();
    let mut queue = VecDeque::new();

    queue.push_back(entry.to_string());
    seen.insert(entry.to_string());

    while let Some(id) = queue.pop_front() {
        if id == join {
            continue;
        }

        nodes.push(id.clone());

        for edge in workflow.edges.iter().filter(|edge| edge.from == id) {
            if edge.to != join && seen.insert(edge.to.clone()) {
                queue.push_back(edge.to.clone());
            }
        }
    }

    nodes
}

fn branch_writes(workflow: &Workflow, branch: &[String], agents: &[AgentDefinition]) -> bool {
    branch.iter().any(|id| {
        let Some(node) = workflow.node(id) else {
            return false;
        };

        match &node.kind {
            NodeKind::Shell { .. } => true,
            NodeKind::Criteria { .. } => true,
            NodeKind::Agent { agent, .. } => agents.iter().any(|definition| {
                &definition.id == agent
                    && definition.allowed_tools.iter().any(|tool| {
                        matches!(tool.as_str(), "write_file" | "edit_file" | "run_command")
                    })
            }),
            _ => false,
        }
    })
}

fn find_uncapped_loop(workflow: &Workflow) -> Option<String> {
    let nodes: Vec<String> = workflow
        .nodes
        .iter()
        .filter(|node| !node.kind.is_human())
        .map(|node| node.id.clone())
        .collect();
    let index: HashMap<String, usize> = nodes
        .iter()
        .enumerate()
        .map(|(index, id)| (id.clone(), index))
        .collect();

    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for edge in &workflow.edges {
        if edge.max_passes.is_some() || edge.pause {
            continue;
        }

        if let (Some(&from), Some(&to)) = (index.get(&edge.from), index.get(&edge.to)) {
            adjacency[from].push(to);
        }
    }

    let mut state = vec![0usize; nodes.len()];
    for start in 0..nodes.len() {
        if state[start] == 0
            && let Some(cycle_node) = dfs_cycle(&adjacency, &mut state, start)
        {
            return Some(nodes[cycle_node].clone());
        }
    }

    None
}

fn dfs_cycle(adjacency: &[Vec<usize>], state: &mut [usize], node: usize) -> Option<usize> {
    state[node] = 1;

    for &next in &adjacency[node] {
        match state[next] {
            0 => {
                if let Some(cycle_node) = dfs_cycle(adjacency, state, next) {
                    return Some(cycle_node);
                }
            }
            1 => return Some(next),
            _ => {}
        }
    }

    state[node] = 2;
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::workflow::definition::{
        AgentContext, Edge, EndOutcome, Node, NodeKind, Position, Workflow,
    };

    fn agent(id: &str, tools: &[&str]) -> AgentDefinition {
        AgentDefinition {
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            subagent: false,
            allowed_tools: tools.iter().map(|tool| tool.to_string()).collect(),
            commands: crate::domain::agent::CommandAccess::ReadWrite,
            writes: crate::domain::agent::WriteScope::Any,
            reasoning_effort: None,
            color: None,
            phase: None,
            prompt: "prompt".to_string(),
            origin: crate::domain::agent::AgentOrigin::Default,
            path: None,
        }
    }

    fn node(id: &str, kind: NodeKind) -> Node {
        Node {
            id: id.to_string(),
            name: None,
            position: Position::default(),
            kind,
        }
    }

    fn edge(from: &str, port: &str, to: &str) -> Edge {
        Edge {
            from: from.to_string(),
            port: port.to_string(),
            to: to.to_string(),
            max_passes: None,
            pause: false,
        }
    }

    fn capped_edge(from: &str, port: &str, to: &str, max_passes: u32) -> Edge {
        Edge {
            from: from.to_string(),
            port: port.to_string(),
            to: to.to_string(),
            max_passes: Some(max_passes),
            pause: false,
        }
    }

    fn paused_edge(from: &str, port: &str, to: &str) -> Edge {
        Edge {
            from: from.to_string(),
            port: port.to_string(),
            to: to.to_string(),
            max_passes: None,
            pause: true,
        }
    }

    fn example_workflow() -> Workflow {
        Workflow {
            id: "plan-build-check".into(),
            name: "Plan, build, check".into(),
            description: String::new(),
            max_steps: 50,
            nodes: vec![
                node("start", NodeKind::Start),
                Node {
                    id: "plan".into(),
                    name: Some("Plan".into()),
                    position: Position::default(),
                    kind: NodeKind::Agent {
                        agent: "plan".into(),
                        prompt: "{{task}}".into(),
                        retry_prompt: None,
                        outputs: vec!["done".into()],
                        context: AgentContext::Session,
                        verdicts: false,
                    },
                },
                Node {
                    id: "approve".into(),
                    name: Some("Approve the plan".into()),
                    position: Position::default(),
                    kind: NodeKind::Human {
                        question: "Go with this plan?".into(),
                        choices: vec!["Go".into(), "Revise".into()],
                    },
                },
                Node {
                    id: "build".into(),
                    name: Some("Build".into()),
                    position: Position::default(),
                    kind: NodeKind::Agent {
                        agent: "builder".into(),
                        prompt: "Execute the plan above.".into(),
                        retry_prompt: Some(
                            "The checks failed:\n\n{{nodes.check.output}}\n\nFix it.".into(),
                        ),
                        outputs: vec!["done".into()],
                        context: AgentContext::Session,
                        verdicts: false,
                    },
                },
                Node {
                    id: "check".into(),
                    name: Some("Checks".into()),
                    position: Position::default(),
                    kind: NodeKind::Shell {
                        command: "just check".into(),
                        timeout_secs: Some(900),
                        success_codes: None,
                    },
                },
                node(
                    "done",
                    NodeKind::End {
                        outcome: EndOutcome::Done,
                    },
                ),
                node(
                    "stuck",
                    NodeKind::End {
                        outcome: EndOutcome::Blocked,
                    },
                ),
            ],
            edges: vec![
                edge("start", "next", "plan"),
                edge("plan", "done", "approve"),
                edge("approve", "Go", "build"),
                edge("approve", "Revise", "plan"),
                edge("build", "done", "check"),
                paused_edge("check", "success", "done"),
                capped_edge("check", "failure", "build", 3),
                edge("check", "exhausted", "stuck"),
            ],
        }
    }

    fn agents() -> Vec<AgentDefinition> {
        vec![
            agent("plan", &["read_file"]),
            agent("builder", &["write_file"]),
        ]
    }

    fn messages(issues: &[Issue]) -> Vec<String> {
        issues.iter().map(|issue| issue.message.clone()).collect()
    }

    #[test]
    fn example_workflow_is_valid() {
        assert!(validate(&example_workflow(), &agents()).is_empty());
    }

    #[test]
    fn missing_start_is_an_issue() {
        let mut workflow = example_workflow();
        workflow.nodes.retain(|node| node.id != "start");

        let issues = validate(&workflow, &agents());
        assert!(messages(&issues).contains(&"missing start node".into()));
    }

    #[test]
    fn unknown_agent_is_an_issue() {
        let mut workflow = example_workflow();
        if let Some(NodeKind::Agent { agent, .. }) = workflow
            .nodes
            .iter_mut()
            .find(|node| node.id == "plan")
            .map(|node| &mut node.kind)
        {
            *agent = "missing".into();
        }

        let issues = validate(&workflow, &agents());
        assert!(messages(&issues).contains(&"unknown agent missing".into()));
    }

    #[test]
    fn sub_agent_in_the_session_context_is_an_issue() {
        let mut agents = agents();
        agents[1].subagent = true;
        let issues = validate(&example_workflow(), &agents);
        assert!(
            messages(&issues).contains(&"sub-agent builder runs only in a fresh context".into())
        );
    }

    #[test]
    fn port_wired_twice_is_an_issue() {
        let mut workflow = example_workflow();
        workflow.edges.push(edge("start", "next", "plan"));

        let issues = validate(&workflow, &agents());
        assert!(messages(&issues).contains(&"port next of start is wired twice".into()));
    }

    #[test]
    fn unknown_template_name_is_an_issue() {
        let mut workflow = example_workflow();
        if let Some(NodeKind::Agent { prompt, .. }) = workflow
            .nodes
            .iter_mut()
            .find(|node| node.id == "plan")
            .map(|node| &mut node.kind)
        {
            *prompt = "{{missing}}".into();
        }

        let issues = validate(&workflow, &agents());
        assert!(messages(&issues).contains(&"unknown template name {{missing}}".into()));
    }

    #[test]
    fn unwired_port_is_an_issue() {
        let mut workflow = example_workflow();
        workflow
            .edges
            .retain(|edge| !(edge.from == "approve" && edge.port == "Revise"));

        let issues = validate(&workflow, &agents());
        assert!(messages(&issues).contains(&"port Revise of approve is not wired".into()));
    }

    #[test]
    fn unreachable_node_is_an_issue() {
        let mut workflow = example_workflow();
        workflow.nodes.push(node(
            "lost",
            NodeKind::End {
                outcome: EndOutcome::Done,
            },
        ));

        let issues = validate(&workflow, &agents());
        assert!(messages(&issues).contains(&"node lost is not reachable from start".into()));
    }

    #[test]
    fn dead_end_is_an_issue() {
        let mut workflow = example_workflow();
        workflow
            .edges
            .retain(|edge| !(edge.from == "check" && edge.port == "success"));
        workflow
            .edges
            .retain(|edge| !(edge.from == "check" && edge.port == "exhausted"));

        let issues = validate(&workflow, &agents());
        assert!(messages(&issues).contains(&"node check cannot reach an end".into()));
    }

    #[test]
    fn parallel_with_two_writing_branches_is_an_issue() {
        let workflow = Workflow {
            id: "w".into(),
            name: "w".into(),
            description: String::new(),
            max_steps: 50,
            nodes: vec![
                node("start", NodeKind::Start),
                node("p", NodeKind::Parallel),
                node(
                    "a",
                    NodeKind::Shell {
                        command: "a".into(),
                        timeout_secs: None,
                        success_codes: None,
                    },
                ),
                node(
                    "b",
                    NodeKind::Shell {
                        command: "b".into(),
                        timeout_secs: None,
                        success_codes: None,
                    },
                ),
                node("j", NodeKind::Join),
                node(
                    "end",
                    NodeKind::End {
                        outcome: EndOutcome::Done,
                    },
                ),
            ],
            edges: vec![
                edge("start", "next", "p"),
                edge("p", "branch", "a"),
                edge("p", "branch", "b"),
                edge("a", "success", "j"),
                edge("b", "success", "j"),
                edge("j", "next", "end"),
            ],
        };

        let issues = validate(&workflow, &agents());
        assert!(messages(&issues).contains(&"only one branch of p may write".into()));
    }

    #[test]
    fn criteria_plan_must_name_an_agent_node() {
        let mut workflow = example_workflow();
        workflow.nodes.push(node(
            "criteria",
            NodeKind::Criteria {
                plan: "missing".into(),
            },
        ));

        let issues = validate(&workflow, &agents());
        assert!(messages(&issues).contains(&"criteria plan missing is not an agent node".into()));
    }

    #[test]
    fn criteria_node_in_a_branch_is_an_issue() {
        let workflow = Workflow {
            id: "w".into(),
            name: "w".into(),
            description: String::new(),
            max_steps: 50,
            nodes: vec![
                node("start", NodeKind::Start),
                node("p", NodeKind::Parallel),
                node(
                    "a",
                    NodeKind::Criteria {
                        plan: "plan".into(),
                    },
                ),
                node(
                    "b",
                    NodeKind::Shell {
                        command: "b".into(),
                        timeout_secs: None,
                        success_codes: None,
                    },
                ),
                node("j", NodeKind::Join),
                node(
                    "end",
                    NodeKind::End {
                        outcome: EndOutcome::Done,
                    },
                ),
            ],
            edges: vec![
                edge("start", "next", "p"),
                edge("p", "branch", "a"),
                edge("p", "branch", "b"),
                edge("a", "pass", "j"),
                edge("a", "fail", "j"),
                edge("b", "success", "j"),
                edge("j", "next", "end"),
            ],
        };

        let issues = validate(&workflow, &agents());
        assert!(
            messages(&issues).contains(&"a criteria node cannot run in a parallel branch".into())
        );
    }

    #[test]
    fn verdicts_agent_in_a_branch_is_an_issue() {
        let workflow = Workflow {
            id: "w".into(),
            name: "w".into(),
            description: String::new(),
            max_steps: 50,
            nodes: vec![
                node("start", NodeKind::Start),
                node("p", NodeKind::Parallel),
                Node {
                    id: "a".into(),
                    name: None,
                    position: Position::default(),
                    kind: NodeKind::Agent {
                        agent: "plan".into(),
                        prompt: "p".into(),
                        retry_prompt: None,
                        outputs: vec!["done".into()],
                        context: AgentContext::Session,
                        verdicts: true,
                    },
                },
                node(
                    "b",
                    NodeKind::Shell {
                        command: "b".into(),
                        timeout_secs: None,
                        success_codes: None,
                    },
                ),
                node("j", NodeKind::Join),
                node(
                    "end",
                    NodeKind::End {
                        outcome: EndOutcome::Done,
                    },
                ),
            ],
            edges: vec![
                edge("start", "next", "p"),
                edge("p", "branch", "a"),
                edge("p", "branch", "b"),
                edge("a", "done", "j"),
                edge("b", "success", "j"),
                edge("j", "next", "end"),
            ],
        };

        let issues = validate(&workflow, &agents());
        assert!(messages(&issues).contains(&"verdicts are recorded by the main walk only".into()));
    }

    #[test]
    fn branch_with_human_node_is_an_issue() {
        let workflow = Workflow {
            id: "w".into(),
            name: "w".into(),
            description: String::new(),
            max_steps: 50,
            nodes: vec![
                node("start", NodeKind::Start),
                node("p", NodeKind::Parallel),
                node(
                    "a",
                    NodeKind::Human {
                        question: "q".into(),
                        choices: vec!["yes".into()],
                    },
                ),
                node(
                    "b",
                    NodeKind::Shell {
                        command: "b".into(),
                        timeout_secs: None,
                        success_codes: None,
                    },
                ),
                node("j", NodeKind::Join),
                node(
                    "end",
                    NodeKind::End {
                        outcome: EndOutcome::Done,
                    },
                ),
            ],
            edges: vec![
                edge("start", "next", "p"),
                edge("p", "branch", "a"),
                edge("p", "branch", "b"),
                edge("a", "yes", "j"),
                edge("b", "success", "j"),
                edge("j", "next", "end"),
            ],
        };

        let issues = validate(&workflow, &agents());
        assert!(messages(&issues).contains(&"branch of p contains a forbidden node a".into()));
    }

    #[test]
    fn uncapped_loop_is_an_issue() {
        let workflow = Workflow {
            id: "w".into(),
            name: "w".into(),
            description: String::new(),
            max_steps: 50,
            nodes: vec![
                node("start", NodeKind::Start),
                node(
                    "a",
                    NodeKind::Shell {
                        command: "a".into(),
                        timeout_secs: None,
                        success_codes: None,
                    },
                ),
                node(
                    "end",
                    NodeKind::End {
                        outcome: EndOutcome::Done,
                    },
                ),
            ],
            edges: vec![
                edge("start", "next", "a"),
                edge("a", "success", "a"),
                edge("a", "failure", "end"),
            ],
        };

        let issues = validate(&workflow, &agents());
        assert!(
            messages(&issues).contains(
                &"this loop has no cap: set maxPasses on one of its edges, pause one, \
             or put a human node on it"
                    .into()
            )
        );
    }

    #[test]
    fn loop_capped_by_max_passes_is_fine() {
        let workflow = Workflow {
            id: "w".into(),
            name: "w".into(),
            description: String::new(),
            max_steps: 50,
            nodes: vec![
                node("start", NodeKind::Start),
                node(
                    "a",
                    NodeKind::Shell {
                        command: "a".into(),
                        timeout_secs: None,
                        success_codes: None,
                    },
                ),
                node(
                    "end",
                    NodeKind::End {
                        outcome: EndOutcome::Done,
                    },
                ),
            ],
            edges: vec![
                edge("start", "next", "a"),
                capped_edge("a", "success", "a", 2),
                edge("a", "failure", "end"),
            ],
        };

        assert!(validate(&workflow, &agents()).is_empty());
    }

    #[test]
    fn loop_through_human_node_is_fine() {
        let workflow = Workflow {
            id: "w".into(),
            name: "w".into(),
            description: String::new(),
            max_steps: 50,
            nodes: vec![
                node("start", NodeKind::Start),
                node(
                    "a",
                    NodeKind::Shell {
                        command: "a".into(),
                        timeout_secs: None,
                        success_codes: None,
                    },
                ),
                node(
                    "h",
                    NodeKind::Human {
                        question: "q".into(),
                        choices: vec!["yes".into()],
                    },
                ),
                node(
                    "end",
                    NodeKind::End {
                        outcome: EndOutcome::Done,
                    },
                ),
            ],
            edges: vec![
                edge("start", "next", "a"),
                edge("a", "success", "h"),
                edge("h", "yes", "a"),
                edge("a", "failure", "end"),
            ],
        };

        assert!(validate(&workflow, &agents()).is_empty());
    }
}
