//! Plugin routes: the human's plugin definitions (`~/.agents/plugins.json`)
//! and their global activations (the Settings page), and the block data the
//! active plugins have emitted. Activation writes the store only.

use super::App;
use super::http::{Request, error, send_json};
use crate::domain::plugin::PluginActivation;
use crate::plugins::config;
use serde_json::{Value, json};
use std::net::TcpStream;

/// `GET /api/plugins`: the global definitions, their activation state, and
/// the global blocks the active plugins have emitted.
pub(super) fn list(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    let store = app.store()?;
    let defs = config::read_defs(&app.agents_home);
    let activations = store.plugin_activations().map_err(|e| e.to_string())?;
    let plugins: Vec<Value> = defs
        .iter()
        .map(|def| {
            let activation = activations.iter().find(|a| a.name == def.name);
            json!({
                "name": def.name,
                "command": def.command,
                "args": def.args,
                "env": def.env,
                "blocks": def.blocks,
                "enabled": activation.is_some_and(|a| a.enabled),
                "network": activation.is_some_and(|a| a.network),
                "active": config::is_active(&app.agents_home, def, activation),
            })
        })
        .collect();
    let blocks = app.plugins.blocks_global();
    send_json(
        stream,
        200,
        &json!({ "plugins": plugins, "blocks": blocks }),
    )
}

/// `GET /api/sessions/{id}/plugins`: the session-scope blocks the session's
/// plugins have emitted, for the left bar.
pub(super) fn session_blocks(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    if store.session(id).map_err(|e| e.to_string())?.is_none() {
        return error(stream, 404, "no such session");
    }
    let blocks = app.plugins.blocks_for_session(id);
    send_json(stream, 200, &json!({ "blocks": blocks }))
}

/// `POST /api/plugins`: set a plugin's activation (enabled, network),
/// hashing its definition and code so a later change deactivates it.
pub(super) fn set(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let body = req.json()?;
    let name = body["name"].as_str().unwrap_or("").to_string();
    let network = body["network"].as_bool().unwrap_or(false);
    let enabled = body["enabled"].as_bool().unwrap_or(false);
    if name.is_empty() {
        return error(stream, 400, "name is empty");
    }
    let store = app.store()?;
    let Some(def) = config::read_defs(&app.agents_home)
        .into_iter()
        .find(|d| d.name == name)
    else {
        return error(stream, 404, "no such plugin");
    };
    if !def.problems().is_empty() {
        return error(stream, 400, &def.problems().join("; "));
    }
    let activation = PluginActivation {
        name: def.name.clone(),
        path: format!("plugins/{}", def.name),
        code_hash: config::compute_hash(&app.agents_home, &def),
        network,
        enabled,
    };
    store
        .save_plugin_activation(&activation)
        .map_err(|e| e.to_string())?;
    // A changed activation starts the hosts afresh, from the pump's thread.
    app.plugins.shutdown();
    app.notifier.notify();
    send_json(
        stream,
        200,
        &json!({
            "name": def.name,
            "enabled": enabled,
            "network": network,
            "active": config::is_active(&app.agents_home, &def, Some(&activation)),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plugin::PluginDef;

    #[test]
    fn a_set_activation_is_stored_with_its_hash() {
        // The route's effect is the store row; the hash ties it to the code.
        let base = std::env::temp_dir().join(format!(
            "fennec-plugin-route-{}",
            crate::domain::id::new_id()
        ));
        let agents_home = base.join("agents");
        std::fs::create_dir_all(agents_home.join("plugins/counter")).unwrap();
        std::fs::write(agents_home.join("plugins/counter/counter.py"), "print(1)").unwrap();
        let def = PluginDef {
            name: "counter".into(),
            command: "python3".into(),
            args: vec!["counter.py".into()],
            env: Default::default(),
            blocks: Vec::new(),
        };
        let hash = config::compute_hash(&agents_home, &def);
        assert!(!hash.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }
}
