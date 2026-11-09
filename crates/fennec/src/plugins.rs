//! The plugin manager: one sandboxed host per activated plugin, for the
//! server's lifetime. Plugins are global: a host sees every session of every
//! project. It starts hosts on demand, feeds them events, and serves the
//! block data they emit. The core renders those blocks; a plugin never
//! writes a session log and nothing of it runs in fennec's process.

pub mod config;
pub mod host;
pub mod pump;

use crate::domain::plugin::{BlockKey, BlockScope, Placement, PluginDef, ServedPluginBlock};
use crate::notifier::Notifier;
use crate::sandbox::Sandbox;
use crate::store::SqliteStore;
use host::Host;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

struct ManagedHost {
    def: PluginDef,
    host: Host,
}

/// The active plugin hosts, one set for the whole server (no per-project
/// map), plus what starting a host needs: the sandbox world, the agents home
/// (`<home>/.agents`) and the plugins' private scratch root.
pub struct PluginManager {
    hosts: Mutex<Option<Vec<ManagedHost>>>,
    sandbox: Sandbox,
    agents_home: PathBuf,
    temp_root: PathBuf,
    /// Moves on every accepted `block.update`, so the notice stream can wake
    /// the UI on plugin data that no session log carries.
    blocks_version: Arc<AtomicU64>,
    notifier: Arc<Notifier>,
}

impl PluginManager {
    pub fn new(
        sandbox: Sandbox,
        agents_home: PathBuf,
        temp_root: PathBuf,
        notifier: Arc<Notifier>,
    ) -> Self {
        Self {
            hosts: Mutex::new(None),
            sandbox,
            agents_home,
            temp_root,
            blocks_version: Arc::new(AtomicU64::new(0)),
            notifier,
        }
    }

    /// The blocks version: moves on every accepted `block.update`.
    pub fn version(&self) -> u64 {
        self.blocks_version.load(Ordering::Relaxed)
    }

    /// Start the active plugin hosts, once. Idempotent: a manager whose
    /// hosts are already up is left alone.
    pub fn ensure_started(&self, store: &SqliteStore) -> Result<(), String> {
        {
            let hosts = crate::locks::lock(&self.hosts);
            if hosts.is_some() {
                return Ok(());
            }
        }
        let defs = config::read_defs(&self.agents_home);
        let activations = store.plugin_activations().map_err(|e| e.to_string())?;
        let mut managed = Vec::new();
        for def in &defs {
            let Some(activation) = activations.iter().find(|a| a.name == def.name) else {
                continue;
            };
            if !config::is_active(&self.agents_home, def, Some(activation)) {
                continue;
            }
            // A plugin's network is its own choice, set on a copy of the world.
            let sandbox = self.sandbox.clone().with_network(activation.network);
            // An accepted update moves the blocks: bump the version and wake
            // the UI's watchers (the notice stream).
            let blocks_version = self.blocks_version.clone();
            let notifier = self.notifier.clone();
            let on_update = Arc::new(move || {
                blocks_version.fetch_add(1, Ordering::Relaxed);
                notifier.notify();
            });
            match Host::spawn(
                def,
                activation.network,
                &sandbox,
                &config::code_dir(&self.agents_home, &def.name),
                &self.temp_root,
                on_update,
            ) {
                Ok(host) => managed.push(ManagedHost {
                    def: def.clone(),
                    host,
                }),
                Err(e) => eprintln!("fennec: {e}"),
            }
        }
        *crate::locks::lock(&self.hosts) = Some(managed);
        Ok(())
    }

    /// Whether any plugin host is running.
    pub fn has_hosts(&self) -> bool {
        crate::locks::lock(&self.hosts)
            .as_ref()
            .is_some_and(|h| !h.is_empty())
    }

    /// Drop every host, after an activation changed; the pump's next wake
    /// starts the active ones afresh. Spawning is left to the pump on purpose:
    /// bwrap's `--die-with-parent` kills a plugin when the *thread* that
    /// spawned it ends, so a host started from a request's thread would die
    /// with the response.
    pub fn shutdown(&self) {
        *crate::locks::lock(&self.hosts) = None;
    }

    /// Feed one event to every host; each host keeps only the types it asked
    /// for. The event carries the `session` and `project` ids, since a host
    /// is global and has no project of its own.
    pub fn push_event(
        &self,
        project_id: &str,
        session_id: &str,
        seq: u64,
        event_type: &str,
        payload: &str,
    ) {
        let hosts = crate::locks::lock(&self.hosts);
        if let Some(managed) = hosts.as_ref() {
            for managed in managed {
                managed
                    .host
                    .push_event(project_id, session_id, seq, event_type, payload);
            }
        }
    }

    /// The global-scope blocks placed in Settings' Plugins tab.
    pub fn blocks_global(&self) -> Vec<ServedPluginBlock> {
        self.blocks(BlockScope::Global, None, None, |placement| {
            *placement == Placement::Settings
        })
    }

    /// The session-scope blocks placed in the session's right-hand context
    /// panel (the shared "Plugins" panel or a panel of their own).
    pub fn blocks_for_session(&self, session_id: &str) -> Vec<ServedPluginBlock> {
        self.blocks(BlockScope::Session, Some(session_id), None, |placement| {
            matches!(placement, Placement::Panel | Placement::OwnPanel)
        })
    }

    /// The project-scope blocks placed in the project page's dashboard.
    pub fn blocks_for_project(&self, project_id: &str) -> Vec<ServedPluginBlock> {
        self.blocks(BlockScope::Project, None, Some(project_id), |placement| {
            *placement == Placement::Dashboard
        })
    }

    /// Every status-bar block, in the order the surfaces stack: the global
    /// ones, then the project's, then the session's.
    pub fn blocks_status_bar(
        &self,
        session_id: Option<&str>,
        project_id: Option<&str>,
    ) -> Vec<ServedPluginBlock> {
        let is_status_bar = |placement: &Placement| *placement == Placement::StatusBar;
        let mut out = self.blocks(BlockScope::Global, None, None, is_status_bar);
        if let Some(project) = project_id {
            out.extend(self.blocks(BlockScope::Project, None, Some(project), is_status_bar));
        }
        if let Some(session) = session_id {
            out.extend(self.blocks(BlockScope::Session, Some(session), None, is_status_bar));
        }
        out
    }

    /// The blocks of one scope, for one key (session, project or neither) and
    /// one placement: a defined block whose stored content exists.
    fn blocks(
        &self,
        scope: BlockScope,
        session_id: Option<&str>,
        project_id: Option<&str>,
        keep: impl Fn(&Placement) -> bool,
    ) -> Vec<ServedPluginBlock> {
        let hosts = crate::locks::lock(&self.hosts);
        let Some(managed) = hosts.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for managed in managed {
            let snapshot = managed.host.blocks_snapshot();
            for block in &managed.def.blocks {
                if block.scope != scope || !keep(&block.placement()) {
                    continue;
                }
                let key = BlockKey {
                    block_id: block.id.clone(),
                    session_id: session_id.map(String::from),
                    project_id: project_id.map(String::from),
                };
                if let Some(content) = snapshot.get(&key) {
                    out.push(ServedPluginBlock {
                        plugin: managed.def.name.clone(),
                        title: block.title.clone(),
                        scope,
                        placement: block.placement(),
                        icon: managed.def.icon,
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
    use crate::notifier::Notifier;
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

    /// A plugin that on `init` emits one block per scope and placement, so a
    /// manager's queries can be checked against real hosts.
    const MULTI: &str = r#"
import json, sys
def send(o):
    sys.stdout.write(json.dumps(o) + "\n"); sys.stdout.flush()
for line in sys.stdin:
    msg = json.loads(line)
    if msg.get("method") == "init":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {"wantedEvents": []}})
        send({"jsonrpc": "2.0", "method": "block.update", "params": {"block": "g", "content": {"elements": [{"kind": "text", "text": "g"}]}}})
        send({"jsonrpc": "2.0", "method": "block.update", "params": {"block": "d", "project": "p1", "content": {"elements": [{"kind": "text", "text": "d"}]}}})
        send({"jsonrpc": "2.0", "method": "block.update", "params": {"block": "pb", "project": "p1", "content": {"elements": [{"kind": "text", "text": "pb"}]}}})
        send({"jsonrpc": "2.0", "method": "block.update", "params": {"block": "s", "session": "s1", "content": {"elements": [{"kind": "text", "text": "s"}]}}})
        send({"jsonrpc": "2.0", "method": "block.update", "params": {"block": "o", "session": "s1", "content": {"elements": [{"kind": "text", "text": "o"}]}}})
        break
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

    /// The agents home holding a plugin's definition and code.
    fn agents_home(base: &Path, name: &str, script: &str, blocks_json: &str) -> PathBuf {
        let agents_home = base.join("agents");
        std::fs::create_dir_all(agents_home.join(format!("plugins/{name}"))).unwrap();
        std::fs::write(
            agents_home.join(format!("plugins/{name}/{name}.py")),
            script,
        )
        .unwrap();
        std::fs::write(
            agents_home.join("plugins.json"),
            format!(
                r#"[{{"name":"{name}","command":"python3","args":["{name}.py"],"blocks":[{blocks_json}]}}]"#
            ),
        )
        .unwrap();
        agents_home
    }

    /// An in-memory store with the named plugin activated on disk.
    fn activated_store(agents_home: &Path, name: &str) -> SqliteStore {
        let store = SqliteStore::in_memory().unwrap();
        let def = config::read_defs(agents_home)
            .into_iter()
            .find(|d| d.name == name)
            .unwrap();
        store
            .save_plugin_activation(&PluginActivation {
                name: name.into(),
                path: format!("plugins/{name}"),
                code_hash: config::compute_hash(agents_home, &def),
                network: false,
                enabled: true,
            })
            .unwrap();
        store
    }

    /// A manager over the given agents home, its hosts started.
    fn started_manager(base: &Path, agents_home: &Path, store: &SqliteStore) -> PluginManager {
        let manager = PluginManager::new(
            sandbox(),
            agents_home.to_path_buf(),
            base.join("scratch"),
            Arc::new(Notifier::default()),
        );
        manager.ensure_started(store).unwrap();
        manager
    }

    /// Poll until `take` finds a value, up to ~3 s of reader-thread latency.
    fn wait_for(mut take: impl FnMut() -> Option<()>) {
        for _ in 0..300 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            if take().is_some() {
                return;
            }
        }
        panic!("the host's blocks never arrived");
    }

    #[test]
    fn plugin_example_counter() {
        let base =
            std::env::temp_dir().join(format!("fennec-plugin-mgr-{}", crate::domain::id::new_id()));
        let agents_home = agents_home(
            &base,
            "counter",
            COUNTER,
            r#"{"id":"calls","title":"Tool calls","scope":"global"}"#,
        );

        let store = activated_store(&agents_home, "counter");
        let manager = started_manager(&base, &agents_home, &store);

        let payload = r#"{"type":"tool.call","data":{}}"#;
        manager.push_event("p", "s", 1, "tool.call", payload);
        manager.push_event("p", "s", 2, "tool.call", payload);

        let mut blocks = Vec::new();
        wait_for(|| {
            blocks = manager.blocks_global();
            blocks
                .first()
                .and_then(|b| b.content.elements.first())
                .and_then(|e| match e {
                    BlockElement::Figure { value, .. } if value != "0" => Some(()),
                    _ => None,
                })
        });
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].plugin, "counter");
        assert_eq!(blocks[0].placement, Placement::Settings);
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
    fn the_queries_serve_only_their_placement_and_scope() {
        let base =
            std::env::temp_dir().join(format!("fennec-plugin-mgr-{}", crate::domain::id::new_id()));
        let agents_home = agents_home(
            &base,
            "multi",
            MULTI,
            r#"{"id":"g","title":"G","scope":"global"},
               {"id":"d","title":"D","scope":"project","placement":"dashboard"},
               {"id":"pb","title":"PB","scope":"project","placement":"status_bar"},
               {"id":"s","title":"S","scope":"session","placement":"status_bar"},
               {"id":"o","title":"O","scope":"session","placement":"own_panel"}"#,
        );

        let store = activated_store(&agents_home, "multi");
        let manager = started_manager(&base, &agents_home, &store);

        wait_for(|| manager.blocks_global().first().map(|_| ()));

        // The Plugins tab takes only the global block placed there.
        let global = manager.blocks_global();
        assert_eq!(global.len(), 1);
        assert_eq!(global[0].title, "G");
        assert_eq!(global[0].placement, Placement::Settings);

        // The dashboard takes only the project block placed there.
        let dashboard = manager.blocks_for_project("p1");
        assert_eq!(dashboard.len(), 1);
        assert_eq!(dashboard[0].title, "D");
        assert_eq!(dashboard[0].placement, Placement::Dashboard);
        assert!(manager.blocks_for_project("p2").is_empty());

        // The context panel takes the panel-placed session blocks, not the
        // status-bar one.
        let panel = manager.blocks_for_session("s1");
        assert_eq!(panel.len(), 1);
        assert_eq!(panel[0].title, "O");
        assert_eq!(panel[0].placement, Placement::OwnPanel);
        assert!(manager.blocks_for_session("s2").is_empty());

        // The status bar stacks global, project, session — here project then
        // session, no global block placed there.
        let bar = manager.blocks_status_bar(Some("s1"), Some("p1"));
        assert_eq!(
            bar.iter().map(|b| b.title.as_str()).collect::<Vec<_>>(),
            vec!["PB", "S"]
        );
        assert!(bar.iter().all(|b| b.placement == Placement::StatusBar));

        manager.shutdown();
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn an_accepted_update_moves_the_blocks_version() {
        let base =
            std::env::temp_dir().join(format!("fennec-plugin-mgr-{}", crate::domain::id::new_id()));
        let agents_home = agents_home(
            &base,
            "multi",
            MULTI,
            r#"{"id":"g","title":"G","scope":"global"}"#,
        );

        let store = activated_store(&agents_home, "multi");
        let manager = started_manager(&base, &agents_home, &store);

        // A new manager starts at 0; the plugin's update may land before any
        // read, and the block is stored just before the version moves, so
        // wait on the version itself.
        wait_for(|| (manager.version() > 0).then_some(()));
        assert!(!manager.blocks_global().is_empty());
        manager.shutdown();
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_shutdown_drops_the_old_hosts() {
        let base =
            std::env::temp_dir().join(format!("fennec-plugin-mgr-{}", crate::domain::id::new_id()));
        let agents_home = agents_home(
            &base,
            "counter",
            COUNTER,
            r#"{"id":"calls","title":"Tool calls","scope":"global"}"#,
        );

        let store = activated_store(&agents_home, "counter");
        let manager = started_manager(&base, &agents_home, &store);
        // The host is alive: an event reaches a block.
        manager.push_event("p", "s", 1, "tool.call", r#"{"type":"tool.call"}"#);
        wait_for(|| manager.blocks_global().first().map(|_| ()));
        manager.shutdown();
        // No host left: the blocks are gone.
        assert!(manager.blocks_global().is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }
}
