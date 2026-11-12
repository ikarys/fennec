//! What a workflow run shows about tokens, from the usage ledger and computed here so
//! the UI only draws it (`GET /api/sessions/{id}/usage/workflow`): one line per node
//! run and a recap per finished run. The log says which calls belong to which node
//! (the node's child session, or the messages between its start and its end); the
//! ledger holds the figures.

use crate::domain::event::Event;
use crate::domain::lang::Lang;
use serde::Serialize;
use std::collections::HashMap;
use token_usage::{CallRecord, display};

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeUsageView {
    /// The thread item it belongs under: `wf-{runId}-{nodeId}-{pass}`.
    pub item_id: String,
    /// `↑ 12,3k · ↓ 345 · 42 tok/s`
    pub line: String,
    pub detail: String,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunUsageView {
    pub run_id: String,
    pub calls: String,
    pub generated: String,
    pub read: String,
    /// `82 % cache`
    pub cache: String,
}

#[derive(Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowUsageView {
    pub nodes: Vec<NodeUsageView>,
    pub runs: Vec<RunUsageView>,
}

/// The child sessions the workflow runs of `events` created.
pub fn child_ids(events: &[Event]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for event in events {
        if let Event::WorkflowNodeStarted {
            child_session_id: Some(child),
            ..
        } = event
            && !ids.contains(child)
        {
            ids.push(child.clone());
        }
    }
    ids
}

#[derive(Default)]
struct Totals {
    calls: u64,
    prompt: u64,
    cached: u64,
    completion: u64,
    secs: f64,
}

impl Totals {
    fn of<'a>(calls: impl IntoIterator<Item = &'a CallRecord>) -> Totals {
        calls.into_iter().fold(Totals::default(), |mut t, c| {
            t.calls += 1;
            t.prompt += c.usage.prompt_tokens;
            t.cached += c.usage.cached_tokens;
            t.completion += c.usage.completion_tokens;
            t.secs += c.total_secs;
            t
        })
    }

    fn speed(&self) -> Option<String> {
        (self.secs > 0.0 && self.completion > 0)
            .then(|| display::speed(self.completion as f64 / self.secs))
    }
}

/// `calls`: the ledger's calls of the session and of each child, by session id.
pub fn workflow_usage(
    session_id: &str,
    events: &[Event],
    calls: &HashMap<String, Vec<CallRecord>>,
    lang: Lang,
) -> WorkflowUsageView {
    let by_message: HashMap<&str, &CallRecord> = calls
        .get(session_id)
        .into_iter()
        .flatten()
        .filter_map(|c| Some((c.labels.get("messageId")?.as_str(), c)))
        .collect();
    // The calls the log shows between two positions of the session.
    let in_session = |from: usize, to: usize| -> Vec<&CallRecord> {
        events[from..to]
            .iter()
            .filter_map(|event| match event {
                Event::MessageDone {
                    message_id,
                    stats: Some(_),
                    partial: false,
                } => by_message.get(message_id.as_str()).copied(),
                _ => None,
            })
            .collect()
    };

    let mut view = WorkflowUsageView::default();
    // (run, node) -> where its last start sits in the log, and its pass and child.
    let mut open: HashMap<(&str, &str), (usize, u32, Option<&str>)> = HashMap::new();
    let mut run_start: HashMap<&str, usize> = HashMap::new();
    let mut run_children: HashMap<&str, Vec<&str>> = HashMap::new();

    for (i, event) in events.iter().enumerate() {
        match event {
            Event::WorkflowStarted { run_id, .. } => {
                run_start.insert(run_id, i);
            }
            Event::WorkflowNodeStarted {
                run_id,
                node_id,
                pass,
                child_session_id,
                ..
            } => {
                open.insert((run_id, node_id), (i, *pass, child_session_id.as_deref()));
                if let Some(child) = child_session_id {
                    run_children.entry(run_id).or_default().push(child);
                }
            }
            Event::WorkflowNodeFinished {
                run_id, node_id, ..
            } => {
                let Some((start, pass, child)) = open.remove(&(run_id.as_str(), node_id.as_str()))
                else {
                    continue;
                };
                let mine: Vec<&CallRecord> = match child {
                    Some(child) => calls.get(child).into_iter().flatten().collect(),
                    None => in_session(start, i),
                };
                if mine.is_empty() {
                    continue;
                }
                let t = Totals::of(mine);
                let speed = t.speed();
                view.nodes.push(NodeUsageView {
                    item_id: format!("wf-{run_id}-{node_id}-{pass}"),
                    line: [
                        Some(format!("↑ {}", display::tokens(t.prompt, lang.locale()))),
                        Some(format!(
                            "↓ {}",
                            display::tokens(t.completion, lang.locale())
                        )),
                        speed.clone(),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · "),
                    detail: match lang {
                        Lang::En => format!(
                            "Input {} tokens, {} cached · output {} tokens · {} call{}",
                            display::tokens(t.prompt, lang.locale()),
                            display::tokens(t.cached, lang.locale()),
                            display::tokens(t.completion, lang.locale()),
                            t.calls,
                            if t.calls > 1 { "s" } else { "" }
                        ),
                        Lang::Fr => format!(
                            "Entrée {} tokens dont {} en cache · sortie {} tokens · {} appel{}",
                            display::tokens(t.prompt, lang.locale()),
                            display::tokens(t.cached, lang.locale()),
                            display::tokens(t.completion, lang.locale()),
                            t.calls,
                            if t.calls > 1 { "s" } else { "" }
                        ),
                    },
                });
            }
            Event::WorkflowFinished { run_id, .. } => {
                let from = run_start.get(run_id.as_str()).copied().unwrap_or(0);
                let mut all = in_session(from, i);
                let mut seen: Vec<&str> = Vec::new();
                for child in run_children.get(run_id.as_str()).into_iter().flatten() {
                    if !seen.contains(child) {
                        seen.push(child);
                        all.extend(calls.get(*child).into_iter().flatten());
                    }
                }
                let t = Totals::of(all);
                view.runs.push(RunUsageView {
                    run_id: run_id.clone(),
                    calls: t.calls.to_string(),
                    generated: display::tokens(t.completion, lang.locale()),
                    read: display::tokens(t.prompt, lang.locale()),
                    cache: format!("{} % cache", display::percent(t.cached, t.prompt)),
                });
            }
            _ => {}
        }
    }
    view
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::stats::MessageStats;
    use token_usage::Usage;

    fn call(message_id: &str, prompt: u64, cached: u64, completion: u64) -> CallRecord {
        CallRecord {
            at_ms: 0,
            model: "m".into(),
            kind: "turn".into(),
            run_id: "s".into(),
            usage: Usage {
                prompt_tokens: prompt,
                cached_tokens: cached,
                completion_tokens: completion,
            },
            total_secs: 2.0,
            prefill_speed: 0.0,
            generation_speed: 0.0,
            labels: [("messageId".to_string(), message_id.to_string())].into(),
        }
    }

    fn done(id: &str) -> Event {
        Event::MessageDone {
            message_id: id.into(),
            stats: Some(MessageStats {
                model: "m".into(),
                total_time: 0.0,
                prefill_tokens: 0,
                cached_prompt_tokens: 0,
                generation_tokens: 0,
                prefill_speed: 0.0,
                generation_speed: 0.0,
            }),
            partial: false,
        }
    }

    fn started(node: &str, child: Option<&str>) -> Event {
        Event::WorkflowNodeStarted {
            run_id: "r".into(),
            node_id: node.into(),
            pass: 1,
            via: None,
            child_session_id: child.map(str::to_string),
            started_at: None,
        }
    }

    fn finished(node: &str) -> Event {
        Event::WorkflowNodeFinished {
            run_id: "r".into(),
            node_id: node.into(),
            port: "done".into(),
            output: String::new(),
            exit_code: None,
            comment: None,
            duration_ms: None,
        }
    }

    #[test]
    fn a_node_counts_its_own_calls_and_a_run_all_of_them() {
        let events = vec![
            started("a", None),
            done("m1"),
            finished("a"),
            started("b", Some("child")),
            finished("b"),
            started("c", None),
            finished("c"),
            Event::WorkflowFinished {
                run_id: "r".into(),
                outcome: crate::domain::workflow::run::RunOutcome::Done,
                reason: None,
                stats: None,
            },
        ];
        let mut calls = HashMap::new();
        calls.insert("s".to_string(), vec![call("m1", 1000, 500, 100)]);
        calls.insert("child".to_string(), vec![call("c1", 3000, 0, 200)]);

        let view = workflow_usage("s", &events, &calls, Lang::Fr);

        let ids: Vec<&str> = view.nodes.iter().map(|n| n.item_id.as_str()).collect();
        assert_eq!(ids, ["wf-r-a-1", "wf-r-b-1"], "c made no call");
        assert!(view.nodes[0].line.starts_with("↑ "));
        assert!(view.nodes[0].detail.contains("1 appel"));
        assert_eq!(view.runs.len(), 1);
        assert_eq!(view.runs[0].calls, "2");
        assert_eq!(view.runs[0].read, display::tokens(4000, Lang::Fr.locale()));
        assert_eq!(view.runs[0].cache, "13 % cache");
    }

    #[test]
    fn a_node_detail_is_english_when_the_language_is_english() {
        let events = vec![started("a", None), done("m1"), finished("a")];
        let mut calls = HashMap::new();
        calls.insert("s".to_string(), vec![call("m1", 1000, 500, 100)]);

        let view = workflow_usage("s", &events, &calls, Lang::En);
        assert_eq!(
            view.nodes[0].detail,
            "Input 1.0k tokens, 500 cached · output 100 tokens · 1 call"
        );
    }
}
