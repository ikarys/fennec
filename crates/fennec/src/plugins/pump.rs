//! The event pump: woken by the notifier (no polling), it drains each
//! project's new events and feeds the ones a plugin asked for to that
//! project's hosts. It reads the store only; a plugin never writes a log.

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
    // A session's last seq already fed to the plugins; a restart re-feeds the
    // tail, which is harmless (a plugin re-derives its block from the count).
    let mut high_water: HashMap<String, u64> = HashMap::new();
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let seen = notifier.generation();
        notifier.wait_past(seen, Duration::from_secs(1));
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let Ok(projects) = store.projects() else {
            continue;
        };
        for project in projects {
            let Ok(activations) = store.plugin_activations(&project.id) else {
                continue;
            };
            if activations.is_empty() {
                continue;
            }
            if manager.ensure_project(&project.id, &store).is_err() {
                continue;
            }
            let Ok(sessions) = store.sessions(Some(&project.id)) else {
                continue;
            };
            for session in sessions {
                let last = high_water.get(&session.id).copied().unwrap_or(0);
                let Ok(events) = store.events_after(&session.id, last) else {
                    continue;
                };
                if events.is_empty() {
                    continue;
                }
                for (seq, payload) in &events {
                    let Some(event_type) =
                        serde_json::from_str::<Value>(payload)
                            .ok()
                            .and_then(|value| {
                                value.get("type").and_then(Value::as_str).map(String::from)
                            })
                    else {
                        continue;
                    };
                    manager.push_event_to_project(
                        &project.id,
                        &session.id,
                        *seq,
                        &event_type,
                        payload,
                    );
                }
                if let Some((max_seq, _)) = events.last() {
                    high_water.insert(session.id.clone(), *max_seq);
                }
            }
        }
    }
}
