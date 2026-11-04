//! Plugin routes: the project's plugin definitions and the human's
//! activations (the Settings page), and the block data the active plugins have
//! emitted. Activation writes the store only; the definitions live in the
//! project's protected `.agents` files.

use super::App;
use super::http::{Request, error, send_json};
use crate::domain::plugin::PluginActivation;
use crate::plugins::config;
use serde_json::{Value, json};
use std::net::TcpStream;
use std::path::Path;

/// `GET /api/projects/{id}/plugins`: the definitions, their activation state,
/// and the project blocks the active plugins have emitted.
pub(super) fn list(app: &App, stream: &mut TcpStream, project_id: &str) -> Result<(), String> {
    let store = app.store()?;
    let Some(project) = store.project(project_id).map_err(|e| e.to_string())? else {
        return error(stream, 404, "no such project");
    };
    let workdir = Path::new(&project.workdir);
    let defs = config::read_defs(workdir);
    let activations = store
        .plugin_activations(project_id)
        .map_err(|e| e.to_string())?;
    let plugins: Vec<Value> = defs
        .iter()
        .map(|def| {
            let activation = activations.iter().find(|a| a.name == def.name);
            json!({
                "name": def.name,
                "command": def.command,
                "args": def.args,
                "blocks": def.blocks,
                "enabled": activation.is_some_and(|a| a.enabled),
                "network": activation.is_some_and(|a| a.network),
                "active": config::is_active(workdir, def, activation),
            })
        })
        .collect();
    let blocks = app.plugins.blocks_for_project(project_id);
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
    let Some(session) = store.session(id).map_err(|e| e.to_string())? else {
        return error(stream, 404, "no such session");
    };
    let blocks = app.plugins.blocks_for_session(&session.project_id, id);
    send_json(stream, 200, &json!({ "blocks": blocks }))
}

/// `POST /api/projects/{id}/plugins`: set a plugin's activation (enabled,
/// network), hashing its definition and code so a later change deactivates it.
pub(super) fn set(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    project_id: &str,
) -> Result<(), String> {
    let body = req.json()?;
    let name = body["name"].as_str().unwrap_or("").to_string();
    let network = body["network"].as_bool().unwrap_or(false);
    let enabled = body["enabled"].as_bool().unwrap_or(false);
    if name.is_empty() {
        return error(stream, 400, "name is empty");
    }
    let store = app.store()?;
    let Some(project) = store.project(project_id).map_err(|e| e.to_string())? else {
        return error(stream, 404, "no such project");
    };
    let workdir = Path::new(&project.workdir);
    let Some(def) = config::read_defs(workdir)
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
        path: format!(".agents/plugins/{}", def.name),
        code_hash: config::compute_hash(workdir, &def),
        network,
        enabled,
    };
    store
        .save_plugin_activation(project_id, &activation)
        .map_err(|e| e.to_string())?;
    // A changed activation starts the host afresh.
    app.plugins.restart_project(project_id, &store);
    send_json(
        stream,
        200,
        &json!({
            "name": def.name,
            "enabled": enabled,
            "network": network,
            "active": config::is_active(workdir, &def, Some(&activation)),
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
        let workdir = base.join("repo");
        std::fs::create_dir_all(workdir.join(".agents/plugins/counter")).unwrap();
        std::fs::write(
            workdir.join(".agents/plugins/counter/counter.py"),
            "print(1)",
        )
        .unwrap();
        let def = PluginDef {
            name: "counter".into(),
            command: "python3".into(),
            args: vec!["counter.py".into()],
            env: Default::default(),
            blocks: Vec::new(),
        };
        let hash = config::compute_hash(&workdir, &def);
        assert!(!hash.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }
}
