//! Plugins that relay the person's decisions from a chat (a Telegram bot...):
//! each enabled plugin with a `serve` program runs for as long as fennec does,
//! with a token of its own. The token may only do what the person was offered
//! on a task (commit, drop, answer, validate "done when"), for a sender the
//! plugin's `allow` names, and each use is written in the task's Journal
//! (rule 1). Disabling the plugin stops it; its token dies with it.
//!
//! The local API is open to the person's own programs anyway (`trusted`):
//! the token does not fence a hostile one, it names who decided and keeps a
//! well-behaved plugin to its few decisions.

use std::io::{self, Read};
use std::process::Child;
use std::sync::{Mutex, OnceLock};

use fennec_os::plugin;
use serde_json::{Value, json};

use crate::state::Store;

/// The only routes a token reaches, with what the Journal says of each.
const DECISIONS: [(&str, &str); 5] = [
    ("/api/land", "commit approved"),
    ("/api/drop", "task dropped"),
    ("/api/answer", "\"done when\" approved"),
    ("/api/say", "answer given"),
    ("/api/reply", "answer given"),
];

struct Relay {
    name: String,
    token: String,
    allow: Vec<String>,
    child: Child,
}

static RELAYS: Mutex<Vec<Relay>> = Mutex::new(Vec::new());
static PORT: OnceLock<u16> = OnceLock::new();

fn token() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Called once the server listens: the relays need its address.
pub fn listen(port: u16) {
    let _ = PORT.set(port);
    sync();
}

/// Each enabled relaying plugin runs, the others stop. One that exited is
/// started again here only, when the person turns it on again: a broken bot
/// never loops.
pub fn sync() {
    let Some(port) = PORT.get() else { return };
    let on = plugin::enabled().unwrap_or_default();
    let wanted: Vec<plugin::Plugin> = plugin::catalog()
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .filter(|p| p.serve.is_some() && on.contains(&p.name))
        .collect();
    let Ok(mut relays) = RELAYS.lock() else {
        return;
    };
    relays.retain_mut(|r| {
        let keep = wanted.iter().any(|p| p.name == r.name);
        if !keep {
            let _ = r.child.kill();
            let _ = r.child.wait();
        }
        keep
    });
    for p in wanted {
        if relays.iter().any(|r| r.name == p.name) {
            continue;
        }
        let started = token().and_then(|t| {
            Ok((
                plugin::start(&p, &format!("http://127.0.0.1:{port}"), &t)?,
                t,
            ))
        });
        match started {
            Ok((child, token)) => relays.push(Relay {
                name: p.name,
                token,
                allow: p.allow,
                child,
            }),
            Err(e) => eprintln!("fennec: plugin {} does not start: {e}", p.name),
        }
    }
}

/// Turning a relaying plugin off and on starts it again if it had exited.
pub fn restart(name: &str) {
    if let Ok(mut relays) = RELAYS.lock() {
        relays.retain_mut(|r| r.name != name || r.child.try_wait().ok().flatten().is_none());
    }
    sync();
}

/// Whether a relaying plugin runs now.
pub fn running(name: &str) -> bool {
    RELAYS.lock().is_ok_and(|mut rs| {
        rs.iter_mut()
            .any(|r| r.name == name && r.child.try_wait().ok().flatten().is_none())
    })
}

/// A call made with a token: which plugin, for whom, and whether it may.
/// `Ok` names the plugin and the sender, for the Journal.
pub fn allowed(
    token: &str,
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<(String, String), &'static str> {
    let relays = RELAYS.lock().map_err(|_| "indisponible")?;
    let r = relays
        .iter()
        .find(|r| r.token == token)
        .ok_or("jeton inconnu")?;
    check(&r.allow, method, path, body).map(|from| (r.name.clone(), from))
}

fn check(allow: &[String], method: &str, path: &str, body: &[u8]) -> Result<String, &'static str> {
    if method != "POST" || !DECISIONS.iter().any(|(p, _)| *p == path) {
        return Err("this plugin may not do that");
    }
    let v: Value = serde_json::from_slice(body).map_err(|_| "corps illisible")?;
    let from = match &v["from"] {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => return Err("sender missing"),
    };
    if !allow.contains(&from) {
        return Err("sender not allowed");
    }
    Ok(from)
}

/// The decision, in the task's Journal: who decided, through which plugin.
pub fn note(store: &Store, plugin: &str, from: &str, path: &str, body: &[u8]) {
    let Some(id) = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|v| v["id"].as_str().map(str::to_string))
    else {
        return;
    };
    let did = DECISIONS
        .iter()
        .find(|(p, _)| *p == path)
        .map_or("", |(_, d)| *d);
    let _ = store.update(&id, |r| {
        if !r["journal"].is_array() {
            r["journal"] = json!([]);
        }
        if let Some(j) = r["journal"].as_array_mut() {
            j.push(json!({"stage": "outside", "via": plugin, "from": from, "did": did}));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::check;

    #[test]
    fn a_token_reaches_only_offered_decisions_for_allowed_senders() {
        let allow = ["42".to_string()];
        let body = br#"{"id": "1", "from": 42}"#;
        assert_eq!(check(&allow, "POST", "/api/land", body), Ok("42".into()));
        assert_eq!(
            check(
                &allow,
                "POST",
                "/api/say",
                br#"{"id": "1", "from": "42", "text": "ok"}"#
            ),
            Ok("42".into())
        );
        assert!(
            check(&allow, "POST", "/api/settings", body).is_err(),
            "never a setting"
        );
        assert!(
            check(&allow, "POST", "/api/rules", body).is_err(),
            "never a rule"
        );
        assert!(check(&allow, "GET", "/api/land", body).is_err());
        assert_eq!(
            check(&allow, "POST", "/api/land", br#"{"id": "1", "from": 7}"#),
            Err("sender not allowed")
        );
        assert_eq!(
            check(&allow, "POST", "/api/land", br#"{"id": "1"}"#),
            Err("sender missing")
        );
        assert!(
            check(&[], "POST", "/api/land", body).is_err(),
            "no list, nobody"
        );
    }
}
