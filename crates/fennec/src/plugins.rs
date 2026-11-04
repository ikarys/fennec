//! The plugin manager: one sandboxed host per activated (project, plugin), for
//! the server's lifetime. It starts hosts on demand, feeds them events, and
//! serves the block data they emit. The core renders those blocks; a plugin
//! never writes a session log and nothing of it runs in fennec's process.

pub mod config;
pub mod host;
pub mod pump;

use crate::domain::plugin::{BlockKey, BlockScope, PluginDef, ServedPluginBlock};
use crate::notifier::Notifier;
use crate::store::SqliteStore;
use crate::tools::Sandbox;
use host::Host;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

struct ManagedHost {
    def: PluginDef,
    host: Host,
}

/// The active plugin hosts, keyed by project id, plus what starting a host
/// needs (the sandbox world and the plugins' private scratch root).
pub struct PluginManager {
    hosts: Mutex<HashMap<String, Vec<ManagedHost>>>,
    sandbox: Sandbox,
    temp_root: PathBuf,
}

impl PluginManager {
    pub fn new(sandbox: Sandbox, temp_root: PathBuf) -> Self {
        Self {
            hosts: Mutex::new(HashMap::new()),
            sandbox,
            temp_root,
        }
    }

    /// Start this project's active plugin hosts, once. Idempotent: a project
    /// whose hosts are already up is left alone.
    pub fn ensure_project(&self, project_id: &str, store: &SqliteStore) -> Result<(), String> {
        {
            let hosts = self.hosts.lock().unwrap();
            if hosts.contains_key(project_id) {
                return Ok(());
            }
        }
        let Some(project) = store.project(project_id).map_err(|e| e.to_string())? else {
            return Ok(());
        };
        let workdir = Path::new(&project.workdir);
        let defs = config::read_defs(workdir);
        let activations = store
            .plugin_activations(project_id)
            .map_err(|e| e.to_string())?;
        let mut managed = Vec::new();
        for def in defs {
            let Some(activation) = activations.iter().find(|a| a.name == def.name) else {
                continue;
            };
            if !config::is_active(workdir, &def, Some(activation)) {
                continue;
            }
            // A plugin's network is its own choice, set on a copy of the world.
            let sandbox = self.sandbox.clone().with_network(activation.network);
            match Host::spawn(
                &def,
                activation.network,
                &sandbox,
                workdir,
                &self.temp_root,
                Arc::new(crate::domain::clock::now_ms),
            ) {
                Ok(host) => managed.push(ManagedHost { def, host }),
                Err(e) => eprintln!("fennec: {e}"),
            }
        }
        self.hosts
            .lock()
            .unwrap()
            .insert(project_id.to_string(), managed);
        Ok(())
    }

    /// Drop and restart a project's hosts, after an activation changed.
    pub fn restart_project(&self, project_id: &str, store: &SqliteStore) {
        self.hosts.lock().unwrap().remove(project_id);
        let _ = self.ensure_project(project_id, store);
    }

    /// Drop a project's hosts, after its deletion (the drop kills the children).
    pub fn remove_project(&self, project_id: &str) {
        self.hosts.lock().unwrap().remove(project_id);
    }

    /// Drop every host; the tests use it to free the child processes before cleaning up.
    #[cfg(test)]
    pub fn shutdown(&self) {
        self.hosts.lock().unwrap().clear();
    }

    /// Feed one event to every host of a project; each host keeps only the
    /// types it asked for.
    pub fn push_event_to_project(
        &self,
        project_id: &str,
        session_id: &str,
        seq: u64,
        event_type: &str,
        payload: &str,
    ) {
        let hosts = self.hosts.lock().unwrap();
        if let Some(managed) = hosts.get(project_id) {
            for managed in managed {
                managed
                    .host
                    .push_event(session_id, seq, event_type, payload);
            }
        }
    }

    /// The project-scope blocks a project's plugins have emitted.
    pub fn blocks_for_project(&self, project_id: &str) -> Vec<ServedPluginBlock> {
        self.blocks(project_id, None)
    }

    /// The session-scope blocks a session's plugins have emitted.
    pub fn blocks_for_session(&self, project_id: &str, session_id: &str) -> Vec<ServedPluginBlock> {
        self.blocks(project_id, Some(session_id))
    }

    fn blocks(&self, project_id: &str, session_id: Option<&str>) -> Vec<ServedPluginBlock> {
        let hosts = self.hosts.lock().unwrap();
        let Some(managed) = hosts.get(project_id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for managed in managed {
            let scope = match session_id {
                Some(_) => BlockScope::Session,
                None => BlockScope::Project,
            };
            let snapshot = managed.host.blocks_snapshot();
            for block in &managed.def.blocks {
                if block.scope != scope {
                    continue;
                }
                let key = BlockKey {
                    block_id: block.id.clone(),
                    session_id: session_id.map(String::from),
                };
                if let Some(content) = snapshot.get(&key) {
                    out.push(ServedPluginBlock {
                        plugin: managed.def.name.clone(),
                        title: block.title.clone(),
                        scope,
                        content: content.clone(),
                    });
                }
            }
        }
        out
    }

    /// Start the event pump on its own thread. The pump is a daemon: it runs
    /// until the process ends, and the hosts are killed when this manager is
    /// dropped.
    pub fn start_pump(
        &self,
        manager: Arc<PluginManager>,
        store_path: &Path,
        notifier: &Arc<Notifier>,
    ) {
        let stop = Arc::new(AtomicBool::new(false));
        let store_path = store_path.to_path_buf();
        let notifier = notifier.clone();
        std::thread::spawn(move || pump::run(manager, store_path, notifier, stop));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plugin::{BlockElement, PluginActivation};
    use crate::domain::preferences::SandboxPrefs;
    use crate::store::SqliteStore;

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

    fn sandbox() -> Sandbox {
        Sandbox::from_prefs(
            &SandboxPrefs {
                enabled: true,
                network: false,
            },
            None,
            &std::env::temp_dir(),
        )
    }

    #[test]
    fn plugin_example_counter() {
        let base =
            std::env::temp_dir().join(format!("fennec-plugin-mgr-{}", crate::domain::id::new_id()));
        let workdir = base.join("repo");
        std::fs::create_dir_all(workdir.join(".agents/plugins/counter")).unwrap();
        std::fs::write(workdir.join(".agents/plugins/counter/counter.py"), COUNTER).unwrap();
        std::fs::write(
            workdir.join(".agents/plugins.json"),
            r#"[{"name":"counter","command":"python3","args":["counter.py"],"blocks":[{"id":"calls","title":"Tool calls","scope":"project"}]}]"#,
        )
        .unwrap();

        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for(workdir.to_str().unwrap()).unwrap();
        // Activate the counter, hashing its current definition and code.
        let def = config::read_defs(&workdir).into_iter().next().unwrap();
        let hash = config::compute_hash(&workdir, &def);
        store
            .save_plugin_activation(
                &project.id,
                &PluginActivation {
                    name: "counter".into(),
                    path: ".agents/plugins/counter".into(),
                    code_hash: hash,
                    network: false,
                    enabled: true,
                },
            )
            .unwrap();

        let manager = PluginManager::new(sandbox(), base.join("scratch"));
        manager.ensure_project(&project.id, &store).unwrap();

        let payload = r#"{"type":"tool.call","data":{}}"#;
        manager.push_event_to_project(&project.id, "s", 1, "tool.call", payload);
        // Past the block's minimum update interval, so the second update lands.
        std::thread::sleep(std::time::Duration::from_millis(
            crate::domain::plugin::MIN_UPDATE_INTERVAL_MS + 20,
        ));
        manager.push_event_to_project(&project.id, "s", 2, "tool.call", payload);

        let mut blocks = Vec::new();
        for _ in 0..300 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            blocks = manager.blocks_for_project(&project.id);
            if blocks
                .first()
                .and_then(|b| b.content.elements.first())
                .is_some_and(|e| matches!(e, BlockElement::Figure { value, .. } if value != "0"))
            {
                break;
            }
        }
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].plugin, "counter");
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

    #[test]
    fn a_removed_project_loses_its_hosts() {
        let base =
            std::env::temp_dir().join(format!("fennec-plugin-mgr-{}", crate::domain::id::new_id()));
        let workdir = base.join("repo");
        std::fs::create_dir_all(workdir.join(".agents/plugins/counter")).unwrap();
        std::fs::write(workdir.join(".agents/plugins/counter/counter.py"), COUNTER).unwrap();
        std::fs::write(
            workdir.join(".agents/plugins.json"),
            r#"[{"name":"counter","command":"python3","args":["counter.py"],"blocks":[{"id":"calls","title":"Tool calls","scope":"project"}]}]"#,
        )
        .unwrap();

        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for(workdir.to_str().unwrap()).unwrap();
        let def = config::read_defs(&workdir).into_iter().next().unwrap();
        store
            .save_plugin_activation(
                &project.id,
                &PluginActivation {
                    name: "counter".into(),
                    path: ".agents/plugins/counter".into(),
                    code_hash: config::compute_hash(&workdir, &def),
                    network: false,
                    enabled: true,
                },
            )
            .unwrap();

        let manager = PluginManager::new(sandbox(), base.join("scratch"));
        manager.ensure_project(&project.id, &store).unwrap();
        // The host is alive: an event reaches a block.
        manager.push_event_to_project(&project.id, "s", 1, "tool.call", r#"{"type":"tool.call"}"#);
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(!manager.blocks_for_project(&project.id).is_empty());
        manager.remove_project(&project.id);
        // No host left: the deleted project's blocks are gone.
        assert!(manager.blocks_for_project(&project.id).is_empty());
        manager.shutdown();
        let _ = std::fs::remove_dir_all(&base);
    }
}
