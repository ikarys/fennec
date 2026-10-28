//! The Settings routes (contract: docs/api.md): the stored preferences and what each provider lists.
//! `routes.rs` only dispatches here.

use super::App;
use super::http::{Request, error, send_json};
use crate::domain::preferences::{LOCAL_PROVIDER, Preferences, Provider};
use llm_providers::{MasterKey, list_models};
use serde_json::{Value, json};
use std::net::TcpStream;

pub fn get_settings(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    let prefs = app.store()?.preferences()?;
    let local = prefs.provider(LOCAL_PROVIDER).cloned().unwrap_or(Provider {
        id: LOCAL_PROVIDER.to_string(),
        ..Default::default()
    });
    let others = prefs.providers.iter().filter(|p| p.id != LOCAL_PROVIDER);
    let providers: Vec<Provider> = std::iter::once(local).chain(others.cloned()).collect();
    // One thread per provider: a slow or dead endpoint costs its own timeout only.
    let listings: Vec<Value> = std::thread::scope(|scope| {
        let handles: Vec<_> = providers
            .iter()
            .map(|provider| scope.spawn(|| listing(app, &prefs, provider)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("a listing thread panicked"))
            .collect()
    });
    let body = json!({
        "preferences": prefs,
        "listings": listings,
        "defaults": app.settings.llm.defaults,
    });
    send_json(stream, 200, &body)
}

/// What `provider` lists now. The local endpoint falls back to the startup
/// listing: it may be down now, or have loaded another model since start.
fn listing(app: &App, prefs: &Preferences, provider: &Provider) -> Value {
    let local = provider.id == LOCAL_PROVIDER;
    let name = match provider.name.as_str() {
        "" if local => "Local",
        "" => &provider.id,
        name => name,
    };
    let (base_url, listed) = match app.settings.endpoint(prefs, &provider.id) {
        Ok(endpoint) => (endpoint.base_url.clone(), list_models(&endpoint)),
        Err(e) => (provider.base_url.clone(), Err(e)),
    };
    let (models, error) = match listed {
        Ok(models) => (models, None),
        Err(e) if local => (app.settings.llm.listed.clone(), Some(e)),
        Err(e) => (Vec::new(), Some(e)),
    };
    json!({
        "provider": provider.id,
        "name": name,
        "baseUrl": base_url,
        "models": models,
        "error": error,
    })
}

pub fn save_settings(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let body = req.json()?;
    let mut prefs: Preferences = match serde_json::from_value(body) {
        Ok(prefs) => prefs,
        Err(e) => return error(stream, 400, &format!("invalid preferences: {e}")),
    };
    prefs.migrate();
    if let Err(e) = prefs.validate() {
        return error(stream, 400, &e);
    }

    if prefs.has_plain_keys() {
        let master = match MasterKey::load_or_create(&app.settings.secret_path()) {
            Ok(master) => master,
            Err(e) => return error(stream, 500, &format!("load the master key: {e}")),
        };
        prefs.seal_keys(|key| master.seal(key));
    }

    app.store()?.save_preferences(&prefs)?;
    get_settings(app, stream)
}
