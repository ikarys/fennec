//! One plugin's process, in the sandbox: a JSON-RPC/stdio observer. fennec
//! feeds it the events it declared it wants and it pushes typed block data
//! back; the core stores that data and renders it. A plugin is always
//! sandboxed — a turned-off sandbox is a refusal to start, not a fallback.

use crate::domain::plugin::{BlockContent, BlockKey, MAX_BLOCKS, PluginBlockUpdate, PluginDef};
use crate::tools::Sandbox;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

/// How long fennec waits for a plugin's `init` reply before giving up.
const INIT_TIMEOUT: Duration = Duration::from_secs(5);

/// A running plugin: its child process and the block data it has emitted.
pub struct Host {
    child: Mutex<Option<Child>>,
    stdin: Mutex<ChildStdin>,
    blocks: Arc<RwLock<HashMap<BlockKey, BlockContent>>>,
    /// The event types the plugin asked for in its `init` reply.
    wanted: BTreeSet<String>,
}

impl Host {
    /// Spawn the plugin in the sandbox world and complete the `init`
    /// handshake. `network` is baked into `sandbox` from the activation. The
    /// world has no project: the plugin's code directory is mounted
    /// read-only and is its working directory, and its private scratch
    /// (`temp_root/<name>`) is the only writable place besides `/tmp`.
    /// `on_update` is called after each accepted `block.update`, so the
    /// caller can wake the UI.
    pub fn spawn(
        def: &PluginDef,
        network: bool,
        sandbox: &Sandbox,
        code_dir: &Path,
        temp_root: &Path,
        on_update: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Host, String> {
        // A plugin is always sandboxed: Off is a refusal, not a bare process.
        let read_only = match sandbox {
            Sandbox::Off => {
                return Err(format!(
                    "plugin {}: the sandbox is off, so it does not start",
                    def.name
                ));
            }
            sandbox => {
                // `read_only` only fails when the sandbox is off, which the
                // match above already refused.
                sandbox
                    .clone()
                    .writing(vec![temp_root.join(&def.name)])
                    .read_only(code_dir)
                    .ok_or_else(|| format!("plugin {}: the sandbox is off", def.name))?
            }
        };

        std::fs::create_dir_all(temp_root.join(&def.name))
            .map_err(|e| format!("plugin {}: scratch dir: {e}", def.name))?;
        let mut cmd = read_only.spawn(&def.command, &def.args, code_dir, code_dir);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (key, value) in &def.env {
            cmd.env(key, value);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("plugin {}: spawn: {e}", def.name))?;
        let mut stdin = child.stdin.take().ok_or("plugin: no stdin")?;
        let stdout = child.stdout.take().ok_or("plugin: no stdout")?;

        let blocks = Arc::new(RwLock::new(HashMap::new()));
        let (init_tx, init_rx) = mpsc::channel();
        let reader_blocks = blocks.clone();
        std::thread::spawn(move || {
            let mut init_sent = false;
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let Ok(value) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if let Some(method) = value.get("method").and_then(Value::as_str) {
                    // A message from the plugin; a stray `id` on it does not
                    // make it a response.
                    if method == "block.update"
                        && let Some(params) = value.get("params")
                        && let Ok(update) =
                            serde_json::from_value::<PluginBlockUpdate>(params.clone())
                        && apply_update(&reader_blocks, &update)
                    {
                        // An accepted update moves the block data: wake the
                        // UI's watchers.
                        on_update();
                    }
                } else if value.get("id").is_some() {
                    // A response. After the `init` reply, one is an answer to a
                    // message fennec did not send: an untrusted plugin may emit
                    // any id, so it is ignored, not a reason to stop reading.
                    if init_sent {
                        continue;
                    }
                    // The `init` reply: hand its result to the spawner.
                    if init_tx
                        .send(value.get("result").cloned().unwrap_or(Value::Null))
                        .is_err()
                    {
                        break;
                    }
                    init_sent = true;
                }
            }
        });

        let init_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "init",
            "params": { "name": def.name, "network": network, "blocks": def.blocks },
        });
        writeln!(stdin, "{init_request}")
            .and_then(|()| stdin.flush())
            .map_err(|e| format!("plugin {}: init: {e}", def.name))?;

        let wanted = match init_rx.recv_timeout(INIT_TIMEOUT) {
            Ok(result) => result
                .get("wantedEvents")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|value| value.as_str().map(String::from))
                .collect(),
            Err(_) => {
                let _ = child.kill();
                return Err(format!("plugin {}: init timed out", def.name));
            }
        };

        Ok(Host {
            child: Mutex::new(Some(child)),
            stdin: Mutex::new(stdin),
            blocks,
            wanted,
        })
    }

    /// Feed one event to the plugin, when it declared it wants that type.
    /// `payload` is the event's stored JSON (its `type` field is the wire
    /// name); the plugin sees the `session` and `project` ids with it, since
    /// it is global and has no project of its own.
    pub fn push_event(
        &self,
        project_id: &str,
        session_id: &str,
        seq: u64,
        event_type: &str,
        payload: &str,
    ) {
        if !self.wanted.contains(event_type) {
            return;
        }
        let message = json!({
            "jsonrpc": "2.0",
            "method": "event",
            "params": {
                "sessionId": session_id,
                "projectId": project_id,
                "seq": seq,
                "type": event_type,
                "event": payload,
            },
        });
        let mut stdin = crate::locks::lock(&self.stdin);
        let _ = writeln!(stdin, "{message}").and_then(|()| stdin.flush());
    }

    /// The block data this plugin has emitted, as a snapshot.
    pub fn blocks_snapshot(&self) -> HashMap<BlockKey, BlockContent> {
        crate::locks::read(&self.blocks).clone()
    }
}

/// Store one capped `block.update`: too large, too full or too many blocks is
/// dropped, so an untrusted plugin cannot grow the server's RAM. Returns
/// whether the update was stored, so the caller can wake the UI only on a
/// change. Frequency is
/// not capped: the line is already parsed, the store is one map insert, and
/// the UI reads a snapshot, so a burst costs nothing and the latest state wins.
fn apply_update(
    blocks: &RwLock<HashMap<BlockKey, BlockContent>>,
    update: &PluginBlockUpdate,
) -> bool {
    if !update.within_caps() {
        return false;
    }
    let key = update.key();
    let mut map = crate::locks::write(blocks);
    if map.len() >= MAX_BLOCKS && !map.contains_key(&key) {
        return false;
    }
    map.insert(key, update.content.clone());
    true
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Some(mut child) = crate::locks::lock(&self.child).take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plugin::{BlockElement, BlockScope};
    use crate::domain::preferences::SandboxPrefs;
    use crate::tools::Sandbox;
    use std::collections::BTreeMap;

    /// The `on_update` hook the tests do not watch: a no-op.
    fn no_update() -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }

    fn temp() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "fennec-plugin-host-{}",
            crate::domain::id::new_id()
        ))
    }

    /// A tiny stdio plugin: on `init` it asks for `tool.call` and emits a block
    /// every time it sees one, counting them.
    const ECHO: &str = r#"
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
        send({"jsonrpc": "2.0", "method": "block.update", "params": {"block": "calls", "content": {"elements": [{"kind": "figure", "label": "calls", "value": str(count)}]}}})
"#;

    fn sandbox() -> Sandbox {
        Sandbox::from_prefs(
            &SandboxPrefs {
                enabled: true,
                network: false,
            },
            None,
            &temp(),
        )
    }

    #[test]
    fn a_host_counts_the_events_it_wants() {
        let agents_home = temp();
        std::fs::create_dir_all(agents_home.join("plugins/echo")).unwrap();
        let script = agents_home.join("plugins/echo/echo.py");
        std::fs::write(&script, ECHO).unwrap();

        let def = PluginDef {
            name: "echo".into(),
            command: "python3".into(),
            args: vec!["echo.py".into()],
            env: Default::default(),
            blocks: vec![crate::domain::plugin::PluginBlockDef {
                id: "calls".into(),
                title: "Calls".into(),
                scope: BlockScope::Global,
                placement: None,
            }],
        };
        let host = Host::spawn(
            &def,
            false,
            &sandbox(),
            &agents_home.join("plugins/echo"),
            &temp(),
            no_update(),
        )
        .unwrap();

        let payload = r#"{"type":"tool.call","data":{}}"#;
        host.push_event("p", "s", 1, "tool.call", payload);
        host.push_event("p", "s", 2, "tool.call", payload);
        // A type it did not ask for is not fed to it.
        host.push_event("p", "s", 3, "message.done", r#"{"type":"message.done"}"#);

        let mut seen = None;
        for _ in 0..200 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            let value = host
                .blocks_snapshot()
                .get(&BlockKey {
                    block_id: "calls".into(),
                    session_id: None,
                    project_id: None,
                })
                .and_then(|content| content.elements.first().cloned())
                .and_then(|element| match element {
                    BlockElement::Figure { value, .. } if value != "0" => Some(value),
                    _ => None,
                });
            if value.is_some() {
                seen = value;
                break;
            }
        }
        assert_eq!(seen, Some("2".to_string()));
        let _ = std::fs::remove_dir_all(&agents_home);
    }

    /// A plugin that puts a stray `id` on its notifications: not a response
    /// fennec asked for, so it must not stop the reader.
    const STRAY_ID: &str = r#"
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
        send({"jsonrpc": "2.0", "id": 99, "method": "block.update", "params": {"block": "calls", "content": {"elements": [{"kind": "figure", "label": "calls", "value": str(count)}]}}})
"#;

    #[test]
    fn a_stray_id_after_the_init_does_not_stop_the_reader() {
        let agents_home = temp();
        std::fs::create_dir_all(agents_home.join("plugins/stray")).unwrap();
        std::fs::write(agents_home.join("plugins/stray/stray.py"), STRAY_ID).unwrap();

        let def = PluginDef {
            name: "stray".into(),
            command: "python3".into(),
            args: vec!["stray.py".into()],
            env: Default::default(),
            blocks: Vec::new(),
        };
        let host = Host::spawn(
            &def,
            false,
            &sandbox(),
            &agents_home.join("plugins/stray"),
            &temp(),
            no_update(),
        )
        .unwrap();

        let payload = r#"{"type":"tool.call","data":{}}"#;
        host.push_event("p", "s", 1, "tool.call", payload);
        host.push_event("p", "s", 2, "tool.call", payload);

        let mut seen = None;
        for _ in 0..200 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            let value = host
                .blocks_snapshot()
                .get(&BlockKey {
                    block_id: "calls".into(),
                    session_id: None,
                    project_id: None,
                })
                .and_then(|content| content.elements.first().cloned())
                .and_then(|element| match element {
                    BlockElement::Figure { value, .. } if value != "0" => Some(value),
                    _ => None,
                });
            if value.is_some() {
                seen = value;
                break;
            }
        }
        assert_eq!(seen, Some("2".to_string()));
        let _ = std::fs::remove_dir_all(&agents_home);
    }

    /// A probe plugin that reports whether it can reach the network, read a
    /// file in fennec's (hidden) store directory, or write a file in a project
    /// directory (the plugin's world has no project).
    const PROBE: &str = r#"
import json, sys, socket, os
def send(o):
    sys.stdout.write(json.dumps(o) + "\n"); sys.stdout.flush()
for line in sys.stdin:
    msg = json.loads(line)
    if msg.get("method") == "init":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {"wantedEvents": []}})
        break
store = os.environ.get("FENNEC_STORE_PROBE", "")
store_visible = 1 if (store and os.path.exists(store)) else 0
project = os.environ.get("FENNEC_PROJECT_PROBE", "")
project_writable = 1 if (project and os.path.exists(project) and os.access(project, os.W_OK)) else 0
try:
    socket.create_connection(("9.9.9.9", 80), timeout=2); net = 1
except Exception:
    net = 0
send({"jsonrpc": "2.0", "method": "block.update", "params": {"block": "probe", "content": {"elements": [{"kind": "figure", "label": "probe", "value": "net=%d store=%d project=%d" % (net, store_visible, project_writable)}]}}})
"#;

    #[test]
    fn plugin_sandbox() {
        let base = temp();
        let agents_home = base.join("agents");
        std::fs::create_dir_all(agents_home.join("plugins/probe")).unwrap();
        std::fs::write(agents_home.join("plugins/probe/probe.py"), PROBE).unwrap();

        // A real file in the data dir the sandbox hides.
        let data_dir = base.join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let store_file = data_dir.join("fennec.db");
        std::fs::write(&store_file, "secret").unwrap();

        // A project directory the plugin's world must not be able to write.
        let workdir = base.join("repo");
        std::fs::create_dir_all(&workdir).unwrap();
        let project_file = workdir.join("file.txt");
        std::fs::write(&project_file, "project").unwrap();

        let mut env = BTreeMap::new();
        env.insert(
            "FENNEC_STORE_PROBE".into(),
            store_file.to_string_lossy().into_owned(),
        );
        env.insert(
            "FENNEC_PROJECT_PROBE".into(),
            project_file.to_string_lossy().into_owned(),
        );
        let def = PluginDef {
            name: "probe".into(),
            command: "python3".into(),
            args: vec!["probe.py".into()],
            env,
            blocks: vec![crate::domain::plugin::PluginBlockDef {
                id: "probe".into(),
                title: "Probe".into(),
                scope: BlockScope::Global,
                placement: None,
            }],
        };
        let sandbox = Sandbox::from_prefs(
            &SandboxPrefs {
                enabled: true,
                network: false,
            },
            None,
            &data_dir,
        );
        let host = Host::spawn(
            &def,
            false,
            &sandbox,
            &agents_home.join("plugins/probe"),
            &base.join("scratch"),
            no_update(),
        )
        .unwrap();

        let mut value = None;
        for _ in 0..300 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            if let Some(content) = host.blocks_snapshot().get(&BlockKey {
                block_id: "probe".into(),
                session_id: None,
                project_id: None,
            }) && let Some(BlockElement::Figure { value: v, .. }) = content.elements.first()
            {
                value = Some(v.clone());
                if value.as_deref() == Some("net=0 store=0 project=0") {
                    break;
                }
            }
        }
        assert_eq!(value, Some("net=0 store=0 project=0".to_string()));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn plugin_requires_sandbox() {
        let code_dir = temp();
        std::fs::create_dir_all(&code_dir).unwrap();
        let def = PluginDef {
            name: "echo".into(),
            command: "python3".into(),
            args: vec!["echo.py".into()],
            env: Default::default(),
            blocks: Vec::new(),
        };
        let off = Sandbox::Off;
        let err = match Host::spawn(&def, false, &off, &code_dir, &temp(), no_update()) {
            Ok(_) => panic!("a plugin starts with the sandbox off"),
            Err(err) => err,
        };
        assert!(err.contains("sandbox is off"));
    }

    #[test]
    fn plugin_block_caps() {
        // Exercise the caps directly on the store side.
        let blocks = Arc::new(RwLock::new(HashMap::new()));
        // A content beyond the element cap is not stored.
        let update = PluginBlockUpdate {
            block: "big".into(),
            session: None,
            project: None,
            content: BlockContent {
                elements: vec![
                    BlockElement::Text { text: "x".into() };
                    crate::domain::plugin::MAX_ELEMENTS + 1
                ],
            },
        };
        assert!(!apply_update(&blocks, &update));
        assert!(crate::locks::read(&blocks).is_empty());

        // Updates in a burst are all stored: the latest wins.
        let text = |value: &str| PluginBlockUpdate {
            block: "calls".into(),
            session: None,
            project: None,
            content: BlockContent {
                elements: vec![BlockElement::Text { text: value.into() }],
            },
        };
        assert!(apply_update(&blocks, &text("1")));
        assert!(apply_update(&blocks, &text("2")));
        assert!(apply_update(&blocks, &text("3")));
        assert_eq!(
            blocks
                .read()
                .unwrap()
                .get(&BlockKey {
                    block_id: "calls".into(),
                    session_id: None,
                    project_id: None
                })
                .unwrap()
                .elements
                .first()
                .cloned(),
            Some(BlockElement::Text { text: "3".into() })
        );
    }
}
