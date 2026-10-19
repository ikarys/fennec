//! Plugins: programs the person keeps in `~/.config/fennec/plugins/<name>/`,
//! told what happens to their tasks (a question, work ready, a failure...).
//! `plugin.json` says what to run and which events it listens to:
//! `{"run": "notify.sh", "events": ["question", "ready"], "description": "..."}`.
//! fennec runs it with the event as JSON on stdin; it does its own network
//! (Apprise, a webhook), so fennec needs no client for each service.
//!
//! A plugin runs on the host with the person's rights, outside any sandbox:
//! it is read from the config dir only, never from a project or a world
//! (rule 2), and runs only once enabled (`~/.config/fennec/plugins.json`).
//! An event listened to as `before_<step>` is a hook: its non-zero exit
//! stops that step.
//!
//! A plugin may also relay the person's decisions from a chat: `"serve":
//! "bot.py"` runs for as long as it is enabled, with `FENNEC_URL` and its
//! own `FENNEC_TOKEN`; `"allow": ["<sender>"]` names who it may speak for.
//! What that token may do is fennec-lead's to limit.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

/// What a plugin listens to when its `plugin.json` names nothing: the moments
/// a task needs the person.
pub const DEFAULT_EVENTS: [&str; 3] = ["question", "ready", "failed"];

/// A plugin that hangs is killed after this; work never waits longer on one.
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct Plugin {
    pub name: String,
    pub description: String,
    pub events: Vec<String>,
    pub run: PathBuf,
    /// What runs for as long as the plugin is enabled, to relay decisions.
    pub serve: Option<PathBuf>,
    /// The senders it may relay decisions from.
    pub allow: Vec<String>,
}

fn root() -> io::Result<PathBuf> {
    Ok(crate::rules::config_dir()?.join("plugins"))
}

fn store() -> io::Result<PathBuf> {
    Ok(crate::rules::config_dir()?.join("plugins.json"))
}

/// Every plugin folder with a readable `plugin.json`, by name. A folder
/// without one is not a plugin; an unreadable one is named with why, so the
/// person sees it rather than a silent gap.
pub fn catalog() -> io::Result<Vec<Result<Plugin, (String, String)>>> {
    let mut dirs: Vec<PathBuf> = match std::fs::read_dir(root()?) {
        Ok(d) => d.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    dirs.sort();
    Ok(dirs
        .iter()
        .filter(|d| d.join("plugin.json").is_file())
        .map(|d| read(d))
        .collect())
}

fn read(dir: &Path) -> Result<Plugin, (String, String)> {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let bad = |why: String| (name.clone(), why);
    let text = std::fs::read_to_string(dir.join("plugin.json")).map_err(|e| bad(e.to_string()))?;
    let v: Value =
        serde_json::from_str(&text).map_err(|e| bad(format!("plugin.json est illisible : {e}")))?;
    let run = v["run"]
        .as_str()
        .filter(|r| !r.is_empty())
        .ok_or_else(|| bad("plugin.json ne dit pas quoi lancer (run)".into()))?;
    let events = match v["events"].as_array() {
        Some(a) => a
            .iter()
            .filter_map(|e| e.as_str().map(str::to_string))
            .collect(),
        None => DEFAULT_EVENTS.map(str::to_string).to_vec(),
    };
    let allow = v["allow"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|a| a.as_str().map_or_else(|| a.to_string(), str::to_string))
        .collect();
    Ok(Plugin {
        name: name.clone(),
        description: v["description"].as_str().unwrap_or("").to_string(),
        events,
        run: dir.join(run),
        serve: v["serve"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| dir.join(s)),
        allow,
    })
}

/// The plugin names the person enabled; none if the file is not there yet.
pub fn enabled() -> io::Result<Vec<String>> {
    let text = match std::fs::read_to_string(store()?) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    serde_json::from_str(&text)
        .map_err(|e| io::Error::other(format!("plugins.json est illisible : {e}")))
}

/// Enables or disables a plugin.
pub fn choose(name: &str, on: bool) -> io::Result<()> {
    let mut names = enabled()?;
    match (on, names.iter().any(|n| n == name)) {
        (true, false) => names.push(name.to_string()),
        (false, true) => names.retain(|n| n != name),
        _ => return Ok(()),
    }
    let path = store()?;
    std::fs::create_dir_all(path.parent().unwrap_or(&path))?;
    std::fs::write(&path, serde_json::to_string_pretty(&names)?)?;
    Ok(())
}

/// Runs one plugin with the event on stdin, from its own folder. Its stderr's
/// end says why it failed.
pub fn call(plugin: &Plugin, event: &Value) -> Result<(), String> {
    let mut child = Command::new(&plugin.run)
        .current_dir(plugin.run.parent().unwrap_or(Path::new("/")))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ne démarre pas : {e}"))?;
    // A plugin that does not read its stdin only loses the event.
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(event.to_string().as_bytes());
    }
    let start = Instant::now();
    // ponytail: stderr is read after the exit; a plugin writing more than a
    // pipe holds (64 KiB) blocks until the timeout kills it.
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() < TIMEOUT => std::thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("arrêté après {} s", TIMEOUT.as_secs()));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    if status.success() {
        return Ok(());
    }
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    let last = err
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    Err(match status.code() {
        Some(c) if last.is_empty() => format!("sortie {c}"),
        Some(c) => format!("sortie {c} : {}", last.trim()),
        None => "tué par un signal".into(),
    })
}

/// Starts a plugin's `serve` program from its folder; the caller keeps the
/// child and stops it when the plugin is disabled. Its stderr goes to
/// fennec's, where its errors are read.
pub fn start(plugin: &Plugin, url: &str, token: &str) -> io::Result<Child> {
    let serve = plugin
        .serve
        .as_ref()
        .ok_or_else(|| io::Error::other("ce plugin ne relaie rien"))?;
    Command::new(serve)
        .current_dir(serve.parent().unwrap_or(Path::new("/")))
        .env("FENNEC_URL", url)
        .env("FENNEC_TOKEN", token)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
}

/// Runs, one after the other, every enabled plugin listening to the event's
/// `event`; what each said back, by name. The caller decides whether to wait
/// (a hook) or not (a notification, in its own thread).
pub fn emit(event: &Value) -> io::Result<Vec<(String, Result<(), String>)>> {
    let name = event["event"].as_str().unwrap_or("");
    let on = enabled()?;
    Ok(catalog()?
        .into_iter()
        .flatten()
        .filter(|p| on.contains(&p.name) && p.events.iter().any(|e| e == name))
        .map(|p| {
            let r = call(&p, event);
            (p.name, r)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_EVENTS, catalog, choose, emit, enabled};
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn sandbox(tag: &str) -> (std::sync::MutexGuard<'static, ()>, std::path::PathBuf) {
        let held = crate::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir =
            std::env::temp_dir().join(format!("fennec-os-plugin-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &dir) };
        (held, dir)
    }

    fn make(dir: &Path, name: &str, manifest: serde_json::Value, script: &str) {
        let folder = dir.join("fennec/plugins").join(name);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("plugin.json"), manifest.to_string()).unwrap();
        let run = folder.join("run.sh");
        std::fs::write(&run, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn only_enabled_plugins_listening_to_the_event_run() {
        let (_held, dir) = sandbox("emit");
        let got = dir.join("got");
        make(
            &dir,
            "a",
            json!({"run": "run.sh"}),
            &format!("cat >> {}", got.display()),
        );
        make(
            &dir,
            "b",
            json!({"run": "run.sh", "events": ["started"]}),
            "exit 0",
        );
        make(&dir, "c", json!({"run": "run.sh"}), "echo boom >&2; exit 3");
        std::fs::create_dir_all(dir.join("fennec/plugins/not-one")).unwrap();
        assert_eq!(catalog().unwrap().len(), 3);
        assert_eq!(
            catalog().unwrap()[0].as_ref().unwrap().events,
            DEFAULT_EVENTS
        );

        // Nothing enabled: nothing runs.
        assert!(emit(&json!({"event": "ready"})).unwrap().is_empty());
        choose("a", true).unwrap();
        choose("b", true).unwrap();
        choose("c", true).unwrap();
        choose("c", true).unwrap();
        assert_eq!(enabled().unwrap(), ["a", "b", "c"]);

        let said = emit(&json!({"event": "ready", "task": "1"})).unwrap();
        assert_eq!(said.len(), 2, "b listens to started only");
        assert_eq!(said[0], ("a".into(), Ok(())));
        assert_eq!(said[1], ("c".into(), Err("sortie 3 : boom".into())));
        let line: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&got).unwrap()).unwrap();
        assert_eq!(line["task"], "1");

        choose("a", false).unwrap();
        assert_eq!(enabled().unwrap(), ["b", "c"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_relaying_plugin_runs_with_its_address_and_token() {
        let (_held, dir) = sandbox("serve");
        let got = dir.join("got");
        make(
            &dir,
            "bot",
            json!({"run": "run.sh", "serve": "run.sh", "allow": ["42", 7]}),
            &format!("echo \"$FENNEC_URL $FENNEC_TOKEN\" > {}", got.display()),
        );
        let bot = catalog().unwrap().remove(0).unwrap();
        assert_eq!(bot.allow, ["42", "7"], "a number is a sender too");
        super::start(&bot, "http://127.0.0.1:1", "t0k")
            .unwrap()
            .wait()
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&got).unwrap(),
            "http://127.0.0.1:1 t0k\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_broken_manifest_is_named_not_skipped() {
        let (_held, dir) = sandbox("bad");
        make(&dir, "x", json!({"events": ["ready"]}), "exit 0");
        let all = catalog().unwrap();
        let (name, why) = all[0].as_ref().unwrap_err();
        assert_eq!(name, "x");
        assert!(why.contains("run"), "{why}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
