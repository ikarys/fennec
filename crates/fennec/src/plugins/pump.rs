//! The event pump: woken by the notifier (no polling), it drains the new
//! events of every session of every project and feeds them to the global
//! hosts. It reads the store only; a plugin never writes a log. A session
//! seen for the first time is not replayed: its high-water starts at its
//! current last seq.

use crate::notifier::Notifier;
use crate::plugins::PluginManager;
use crate::store::SqliteStore;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Run the pump on its own thread until `stop` is set.
pub fn run(
    manager: Arc<PluginManager>,
    store_path: std::path::PathBuf,
    notifier: Arc<Notifier>,
    stop: Arc<AtomicBool>,
) {
    let Ok(store) = SqliteStore::open(&store_path) else {
        return;
    };
    let mut high_water = HashMap::new();
    // The first pass starts the active plugins; later ones wait for an append.
    let mut seen = None;
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if let Some(generation) = seen {
            notifier.wait_past(generation, Duration::from_secs(1));
        }
        if stop.load(Ordering::Relaxed) {
            break;
        }
        // The timeout only checks `stop`: scanning every session each second
        // with nothing appended cost an idle server a few percent of a core.
        let generation = notifier.generation();
        if seen == Some(generation) {
            continue;
        }
        seen = Some(generation);
        step(&manager, &store, &mut high_water);
    }
}

/// One wake: feed the sessions' new events. `high_water` holds each
/// session's last seq already fed. With no active plugin it reads nothing, so
/// a server without plugins pays no query per wake.
pub(crate) fn step(
    manager: &PluginManager,
    store: &SqliteStore,
    high_water: &mut HashMap<String, u64>,
) {
    if manager.ensure_started(store).is_err() || !manager.has_hosts() {
        // A plugin activated later starts from then on, not from here.
        high_water.clear();
        return;
    }
    let Ok(projects) = store.projects() else {
        return;
    };
    for project in projects {
        let Ok(sessions) = store.sessions(Some(&project.id)) else {
            continue;
        };
        for session in sessions {
            match high_water.get(&session.id) {
                // A session seen for the first time is not replayed: only
                // events after its current last seq reach the plugins.
                None => {
                    if let Ok(last) = store.last_seq(&session.id) {
                        high_water.insert(session.id.clone(), last);
                    }
                }
                Some(last) => {
                    let Ok(events) = store.events_after(&session.id, *last) else {
                        continue;
                    };
                    if events.is_empty() {
                        continue;
                    }
                    for (seq, payload) in &events {
                        let Some(event_type) = serde_json::from_str::<Value>(payload)
                            .ok()
                            .and_then(|value| {
                                value.get("type").and_then(Value::as_str).map(String::from)
                            })
                        else {
                            continue;
                        };
                        manager.push_event(&project.id, &session.id, *seq, &event_type, payload);
                    }
                    if let Some((max_seq, _)) = events.last() {
                        high_water.insert(session.id.clone(), *max_seq);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ports::EventLog;
    use crate::domain::event::Event;
    use crate::domain::plugin::{BlockElement, PluginActivation};
    use crate::domain::preferences::SandboxPrefs;
    use crate::plugins::config;
    use crate::store::SqliteStore;
    use crate::tools::Sandbox;

    /// The example counter plugin's script: counts `tool.call` events.
    const COUNTER: &str = r#"
import json, sys
count = 0
def send(o):
    sys.stdout.write(json.dumps(o) + "\n"); sys.stdout.flush()
for line in sys.stdin:
    msg = json.loads(line)
    if msg.get("method") == "init":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {"wantedEvents": ["tool.call"]}})
    elif msg.get("method") == "event" and msg["params"]["type"] == "tool.call":
        count += 1
        send({"jsonrpc": "2.0", "method": "block.update", "params": {"block": "calls", "content": {"elements": [{"kind": "figure", "label": "Tool calls", "value": str(count)}]}}})
"#;

    fn call() -> Event {
        Event::ToolCall {
            message_id: "m".into(),
            tool_call: crate::domain::event::ToolCall {
                id: "1".into(),
                name: "run_command".into(),
                arguments: serde_json::json!({}),
                parse_error: None,
                raw_arguments: None,
            },
        }
    }

    /// A session seen for the first time is not replayed: its high-water
    /// starts at its current last seq, so only new events reach the plugins.
    #[test]
    fn a_session_seen_for_the_first_time_is_not_replayed() {
        let base = std::env::temp_dir().join(format!(
            "fennec-plugin-pump-{}",
            crate::domain::id::new_id()
        ));
        let agents_home = base.join("agents");
        std::fs::create_dir_all(agents_home.join("plugins/counter")).unwrap();
        std::fs::write(agents_home.join("plugins/counter/counter.py"), COUNTER).unwrap();
        std::fs::write(
            agents_home.join("plugins.json"),
            r#"[{"name":"counter","command":"python3","args":["counter.py"],"blocks":[{"id":"calls","title":"Tool calls","scope":"global"}]}]"#,
        )
        .unwrap();

        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        let session = store.create_session(&project, "s").unwrap();
        for _ in 0..3 {
            store.append(&session, &call()).unwrap();
        }

        let manager = PluginManager::new(
            Sandbox::from_prefs(
                &SandboxPrefs {
                    enabled: true,
                    network: false,
                },
                None,
                &std::env::temp_dir(),
            ),
            agents_home.clone(),
            base.join("scratch"),
            Arc::new(crate::notifier::Notifier::default()),
        );
        store
            .save_plugin_activation(&PluginActivation {
                name: "counter".into(),
                path: "plugins/counter".into(),
                code_hash: config::compute_hash(
                    &agents_home,
                    &config::read_defs(&agents_home).into_iter().next().unwrap(),
                ),
                network: false,
                enabled: true,
            })
            .unwrap();

        let mut high_water = HashMap::new();
        step(&manager, &store, &mut high_water);
        // The history (the `session.initialized` and three calls) is behind
        // the high-water, not replayed.
        assert_eq!(high_water.get(&session), Some(&4));
        assert!(manager.blocks_global().is_empty());

        // New events after the first sight are fed, one per wake.
        store.append(&session, &call()).unwrap();
        step(&manager, &store, &mut high_water);
        store.append(&session, &call()).unwrap();
        step(&manager, &store, &mut high_water);

        let mut blocks = Vec::new();
        for _ in 0..300 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            blocks = manager.blocks_global();
            if blocks
                .first()
                .and_then(|b| b.content.elements.first())
                .is_some_and(|e| matches!(e, BlockElement::Figure { value, .. } if value != "0"))
            {
                break;
            }
        }
        // Exactly the two new events, not the three that predate the pump.
        assert_eq!(
            blocks[0].content.elements.first().cloned(),
            Some(BlockElement::Figure {
                label: "Tool calls".into(),
                value: "2".into(),
                tone: None,
            })
        );
        manager.shutdown();
        let _ = std::fs::remove_dir_all(&base);
    }
}
