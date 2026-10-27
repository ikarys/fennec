//! Workflow definitions, the graph a human draws. fennec's own; openfox's
//! `workflows/types.ts` inspired the definition.

use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

pub const EXHAUSTED: &str = "exhausted";
pub const BRANCH: &str = "branch";
pub const DEFAULT_SHELL_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_max_steps")]
    pub max_steps: u32,
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub position: Position,
    #[serde(flatten)]
    pub kind: NodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Default for Position {
    fn default() -> Self {
        Position { x: 0.0, y: 0.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum NodeKind {
    Start,
    Agent {
        agent: String,
        #[serde(default)]
        prompt: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retry_prompt: Option<String>,
        #[serde(default = "default_outputs")]
        outputs: Vec<String>,
        #[serde(default)]
        context: AgentContext,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        verdicts: bool,
    },
    Shell {
        command: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_secs: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        success_codes: Option<Vec<i32>>,
    },
    Criteria {
        plan: String,
    },
    Human {
        question: String,
        choices: Vec<String>,
    },
    Condition {
        value: String,
        test: ConditionTest,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        against: Option<String>,
    },
    Parallel,
    Join,
    End {
        outcome: EndOutcome,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentContext {
    #[default]
    Session,
    Fresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConditionTest {
    Equals,
    Contains,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EndOutcome {
    Done,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub from: String,
    pub port: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_passes: Option<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pause: bool,
}

fn default_max_steps() -> u32 {
    50
}

fn default_outputs() -> Vec<String> {
    vec!["done".to_string()]
}

impl Workflow {
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn start(&self) -> Option<&Node> {
        self.nodes
            .iter()
            .find(|node| matches!(node.kind, NodeKind::Start))
    }

    pub fn edge(&self, from: &str, port: &str) -> Option<&Edge> {
        self.edges
            .iter()
            .find(|edge| edge.from == from && edge.port == port)
    }

    pub fn edges_from(&self, from: &str, port: &str) -> impl Iterator<Item = &Edge> {
        self.edges
            .iter()
            .filter(move |edge| edge.from == from && edge.port == port)
    }

    pub fn ports(&self, node: &Node) -> Vec<String> {
        let mut ports = match &node.kind {
            NodeKind::Start => vec!["next".to_string()],
            NodeKind::Agent { outputs, .. } => outputs.clone(),
            NodeKind::Shell { .. } => vec!["success".to_string(), "failure".to_string()],
            NodeKind::Criteria { .. } => vec!["pass".to_string(), "fail".to_string()],
            NodeKind::Human { choices, .. } => choices.clone(),
            NodeKind::Condition { .. } => vec!["true".to_string(), "false".to_string()],
            NodeKind::Parallel => vec![BRANCH.to_string()],
            NodeKind::Join => vec!["next".to_string()],
            NodeKind::End { .. } => Vec::new(),
        };

        let has_capped_edge = self
            .edges
            .iter()
            .any(|edge| edge.from == node.id && edge.max_passes.is_some());
        if has_capped_edge && !ports.iter().any(|port| port == EXHAUSTED) {
            ports.push(EXHAUSTED.to_string());
        }

        ports
    }

    pub fn join_of(&self, parallel_id: &str) -> Option<String> {
        let mut queue = VecDeque::new();
        let mut seen = HashSet::new();

        for edge in self.edges_from(parallel_id, BRANCH) {
            if seen.insert(edge.to.clone()) {
                queue.push_back(edge.to.clone());
            }
        }

        while let Some(id) = queue.pop_front() {
            if self
                .node(&id)
                .is_some_and(|node| matches!(node.kind, NodeKind::Join))
            {
                return Some(id);
            }

            for edge in self.edges.iter().filter(|edge| edge.from == id) {
                if seen.insert(edge.to.clone()) {
                    queue.push_back(edge.to.clone());
                }
            }
        }

        None
    }
}

impl Node {
    pub fn label(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

impl NodeKind {
    pub fn is_end(&self) -> bool {
        matches!(self, NodeKind::End { .. })
    }

    pub fn is_parallel(&self) -> bool {
        matches!(self, NodeKind::Parallel)
    }

    pub fn is_join(&self) -> bool {
        matches!(self, NodeKind::Join)
    }

    pub fn is_human(&self) -> bool {
        matches!(self, NodeKind::Human { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, kind: NodeKind) -> Node {
        Node {
            id: id.to_string(),
            name: None,
            position: Position::default(),
            kind,
        }
    }

    #[test]
    fn ports_include_exhausted_only_for_capped_edges() {
        let workflow = Workflow {
            id: "w".into(),
            name: "w".into(),
            description: String::new(),
            max_steps: 50,
            nodes: vec![node(
                "a",
                NodeKind::Shell {
                    command: "cmd".into(),
                    timeout_secs: None,
                    success_codes: None,
                },
            )],
            edges: vec![Edge {
                from: "a".into(),
                port: "failure".into(),
                to: "a".into(),
                max_passes: Some(1),
                pause: false,
            }],
        };

        let ports = workflow.ports(workflow.node("a").unwrap());
        assert_eq!(ports, vec!["success", "failure", "exhausted"]);
    }

    #[test]
    fn join_of_finds_the_first_join_by_breadth_first_search() {
        let workflow = Workflow {
            id: "w".into(),
            name: "w".into(),
            description: String::new(),
            max_steps: 50,
            nodes: vec![
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
            ],
            edges: vec![
                Edge {
                    from: "p".into(),
                    port: BRANCH.into(),
                    to: "a".into(),
                    max_passes: None,
                    pause: false,
                },
                Edge {
                    from: "p".into(),
                    port: BRANCH.into(),
                    to: "b".into(),
                    max_passes: None,
                    pause: false,
                },
                Edge {
                    from: "a".into(),
                    port: "success".into(),
                    to: "j".into(),
                    max_passes: None,
                    pause: false,
                },
                Edge {
                    from: "b".into(),
                    port: "success".into(),
                    to: "j".into(),
                    max_passes: None,
                    pause: false,
                },
            ],
        };

        assert_eq!(workflow.join_of("p"), Some("j".into()));
    }
}
