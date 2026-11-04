//! One plugin's process, in the sandbox: a JSON-RPC/stdio observer. fennec
//! feeds it the events it declared it wants and it pushes typed block data
//! back; the core stores that data and renders it. A plugin is always
//! sandboxed — a turned-off sandbox is a refusal to start, not a fallback.

use crate::domain::plugin::{
    BlockContent, BlockKey, MAX_BLOCKS, MIN_UPDATE_INTERVAL_MS, PluginBlockUpdate, PluginDef,
};
use crate::plugins::config::code_dir;
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
    /// Spawn the plugin in the sandbox world and complete the `init` handshake.
    /// `network` is baked into `sandbox` from the activation; `temp_root` holds
    /// the plugin's private, writable scratch (outside the project workdir).
    pub fn spawn(
        def: &PluginDef,
        network: bool,
        sandbox: &Sandbox,
        workdir: &Path,
        temp_root: &Path,
        clock: Arc<dyn Fn() -> u64 + Send + Sync>,
    ) -> Result<Host, String> {
        // A plugin is always sandboxed: Off is a refusal, not a bare process.
        let read_only = match sandbox {
            Sandbox::Off => {
                return Err(format!(
                    "plugin {}: the sandbox is off, so it does not start",
                    def.name
                ));
            }
            sandbox => sandbox
                .read_only(workdir)
                .ok_or_else(|| format!("plugin {}: the sandbox is off", def.name))?,
        };

        let temp = temp_root.join(&def.name);
        std::fs::create_dir_all(&temp)
            .map_err(|e| format!("plugin {}: scratch dir: {e}", def.name))?;
        let sandbox = read_only.writing(vec![temp]);

        let code_dir = code_dir(workdir, &def.name);
        let mut cmd = sandbox.spawn(&def.command, &def.args, workdir, &code_dir);
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
        let reader_last = Mutex::new(HashMap::new());
        let reader_clock = clock;
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
                    {
                        apply_update(&reader_blocks, &reader_last, reader_clock(), &update);
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
    /// `payload` is the event's stored JSON (its `type` field is the wire name).
    pub fn push_event(&self, session_id: &str, seq: u64, event_type: &str, payload: &str) {
        if !self.wanted.contains(event_type) {
            return;
        }
        let message = json!({
            "jsonrpc": "2.0",
            "method": "event",
            "params": { "sessionId": session_id, "seq": seq, "type": event_type, "event": payload },
        });
        let mut stdin = self.stdin.lock().unwrap();
        let _ = writeln!(stdin, "{message}").and_then(|()| stdin.flush());
    }

    /// The block data this plugin has emitted, as a snapshot.
    pub fn blocks_snapshot(&self) -> HashMap<BlockKey, BlockContent> {
        self.blocks.read().unwrap().clone()
    }
}

/// Store one capped `block.update`: too large, too full, too many blocks, or
/// too frequent is dropped, so an untrusted plugin cannot grow the server's
/// RAM or flood it. `now` is the clock's millisecond reading.
fn apply_update(
    blocks: &RwLock<HashMap<BlockKey, BlockContent>>,
    last: &Mutex<HashMap<BlockKey, u64>>,
    now: u64,
    update: &PluginBlockUpdate,
) {
    if !update.within_caps() {
        return;
    }
    let key = update.key();
    if last
        .lock()
        .unwrap()
        .get(&key)
        .is_some_and(|since| now.saturating_sub(*since) < MIN_UPDATE_INTERVAL_MS)
    {
        return;
    }
    let mut map = blocks.write().unwrap();
    if map.len() >= MAX_BLOCKS && !map.contains_key(&key) {
        return;
    }
    map.insert(key.clone(), update.content.clone());
    last.lock().unwrap().insert(key, now);
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.lock().unwrap().take() {
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

    fn temp() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "fennec-plugin-host-{}",
            crate::domain::id::new_id()
        ))
    }

    fn clock() -> Arc<dyn Fn() -> u64 + Send + Sync> {
        Arc::new(crate::domain::clock::now_ms)
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
        let workdir = temp();
        std::fs::create_dir_all(workdir.join(".agents/plugins/echo")).unwrap();
        let script = workdir.join(".agents/plugins/echo/echo.py");
        std::fs::write(&script, ECHO).unwrap();

        let def = PluginDef {
            name: "echo".into(),
            command: "python3".into(),
            args: vec!["echo.py".into()],
            env: Default::default(),
            blocks: vec![crate::domain::plugin::PluginBlockDef {
                id: "calls".into(),
                title: "Calls".into(),
                scope: BlockScope::Project,
            }],
        };
        let host = Host::spawn(&def, false, &sandbox(), &workdir, &temp(), clock()).unwrap();

        let payload = r#"{"type":"tool.call","data":{}}"#;
        host.push_event("s", 1, "tool.call", payload);
        // Past the block's minimum update interval, so the second update lands.
        std::thread::sleep(std::time::Duration::from_millis(
            MIN_UPDATE_INTERVAL_MS + 20,
        ));
        host.push_event("s", 2, "tool.call", payload);
        // A type it did not ask for is not fed to it.
        host.push_event("s", 3, "message.done", r#"{"type":"message.done"}"#);

        let mut seen = None;
        for _ in 0..200 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            let value = host
                .blocks_snapshot()
                .get(&BlockKey {
                    block_id: "calls".into(),
                    session_id: None,
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
        let _ = std::fs::remove_dir_all(&workdir);
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
        let workdir = temp();
        std::fs::create_dir_all(workdir.join(".agents/plugins/stray")).unwrap();
        std::fs::write(workdir.join(".agents/plugins/stray/stray.py"), STRAY_ID).unwrap();

        let def = PluginDef {
            name: "stray".into(),
            command: "python3".into(),
            args: vec!["stray.py".into()],
            env: Default::default(),
            blocks: Vec::new(),
        };
        let host = Host::spawn(&def, false, &sandbox(), &workdir, &temp(), clock()).unwrap();

        let payload = r#"{"type":"tool.call","data":{}}"#;
        host.push_event("s", 1, "tool.call", payload);
        std::thread::sleep(std::time::Duration::from_millis(
            MIN_UPDATE_INTERVAL_MS + 20,
        ));
        host.push_event("s", 2, "tool.call", payload);

        let mut seen = None;
        for _ in 0..200 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            let value = host
                .blocks_snapshot()
                .get(&BlockKey {
                    block_id: "calls".into(),
                    session_id: None,
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
        let _ = std::fs::remove_dir_all(&workdir);
    }

    /// A probe plugin that reports whether it can reach the network and read a
    /// file in fennec's (hidden) store directory.
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
try:
    socket.create_connection(("9.9.9.9", 80), timeout=2); net = 1
except Exception:
    net = 0
send({"jsonrpc": "2.0", "method": "block.update", "params": {"block": "probe", "content": {"elements": [{"kind": "figure", "label": "probe", "value": "net=%d store=%d" % (net, store_visible)}]}}})
"#;

    #[test]
    fn plugin_sandbox() {
        let base = temp();
        let workdir = base.join("repo");
        std::fs::create_dir_all(workdir.join(".agents/plugins/probe")).unwrap();
        std::fs::write(workdir.join(".agents/plugins/probe/probe.py"), PROBE).unwrap();

        // A real file in the data dir the sandbox hides.
        let data_dir = base.join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let store_file = data_dir.join("fennec.db");
        std::fs::write(&store_file, "secret").unwrap();

        let mut env = BTreeMap::new();
        env.insert(
            "FENNEC_STORE_PROBE".into(),
            store_file.to_string_lossy().into_owned(),
        );
        let def = PluginDef {
            name: "probe".into(),
            command: "python3".into(),
            args: vec!["probe.py".into()],
            env,
            blocks: vec![crate::domain::plugin::PluginBlockDef {
                id: "probe".into(),
                title: "Probe".into(),
                scope: BlockScope::Project,
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
            &workdir,
            &base.join("scratch"),
            clock(),
        )
        .unwrap();

        let mut value = None;
        for _ in 0..300 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            if let Some(content) = host.blocks_snapshot().get(&BlockKey {
                block_id: "probe".into(),
                session_id: None,
            }) && let Some(BlockElement::Figure { value: v, .. }) = content.elements.first()
            {
                value = Some(v.clone());
                if value.as_deref() == Some("net=0 store=0") {
                    break;
                }
            }
        }
        assert_eq!(value, Some("net=0 store=0".to_string()));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn plugin_requires_sandbox() {
        let workdir = temp();
        std::fs::create_dir_all(&workdir).unwrap();
        let def = PluginDef {
            name: "echo".into(),
            command: "python3".into(),
            args: vec!["echo.py".into()],
            env: Default::default(),
            blocks: Vec::new(),
        };
        let off = Sandbox::Off;
        let err = match Host::spawn(&def, false, &off, &workdir, &temp(), clock()) {
            Ok(_) => panic!("a plugin starts with the sandbox off"),
            Err(err) => err,
        };
        assert!(err.contains("sandbox is off"));
    }

    #[test]
    fn plugin_block_caps() {
        // Exercise the caps directly on the store side.
        let blocks = Arc::new(RwLock::new(HashMap::new()));
        let last = Mutex::new(HashMap::new());
        // A content beyond the element cap is not stored.
        let update = PluginBlockUpdate {
            block: "big".into(),
            session: None,
            content: BlockContent {
                elements: vec![
                    BlockElement::Text { text: "x".into() };
                    crate::domain::plugin::MAX_ELEMENTS + 1
                ],
            },
        };
        apply_update(&blocks, &last, 0, &update);
        assert!(blocks.read().unwrap().is_empty());

        // A too-frequent update on the same block is dropped; one after the
        // minimum interval is stored.
        let text = |value: &str| PluginBlockUpdate {
            block: "calls".into(),
            session: None,
            content: BlockContent {
                elements: vec![BlockElement::Text { text: value.into() }],
            },
        };
        apply_update(&blocks, &last, 0, &text("1"));
        apply_update(&blocks, &last, MIN_UPDATE_INTERVAL_MS - 1, &text("2"));
        {
            let stored = blocks.read().unwrap();
            assert_eq!(
                stored.get(&BlockKey {
                    block_id: "calls".into(),
                    session_id: None
                }),
                Some(&BlockContent {
                    elements: vec![BlockElement::Text { text: "1".into() }]
                })
            );
        }
        apply_update(&blocks, &last, MIN_UPDATE_INTERVAL_MS, &text("3"));
        assert_eq!(
            blocks
                .read()
                .unwrap()
                .get(&BlockKey {
                    block_id: "calls".into(),
                    session_id: None
                })
                .unwrap()
                .elements
                .first()
                .cloned(),
            Some(BlockElement::Text { text: "3".into() })
        );
    }
}
