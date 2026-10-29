//! The Settings routes (contract: docs/api.md): the stored preferences and what each provider lists.
//! `routes.rs` only dispatches here.

use super::App;
use super::http::{Request, error, send_json};
use super::model_view;
use crate::domain::preferences::{LOCAL_PROVIDER, Preferences, Provider};
use llm_providers::{
    ApiKey, Endpoint, ListedModel, MasterKey, ModelParams, ModelPrefs, list_models,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
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

/// What `provider` lists now, at its stored endpoint.
fn listing(app: &App, prefs: &Preferences, provider: &Provider) -> Value {
    let name = provider_name(prefs, &provider.id);
    let (base_url, listed) = match app.settings.endpoint(prefs, &provider.id) {
        Ok(endpoint) => (endpoint.base_url.clone(), list_models(&endpoint)),
        Err(e) => (provider.base_url.clone(), Err(e)),
    };
    listing_body(app, prefs, &provider.id, &name, base_url, listed)
}

/// The name a provider shows in the UI: a stored name, else "Local" for the
/// local provider, else the provider id.
fn provider_name(prefs: &Preferences, provider_id: &str) -> String {
    match prefs.provider(provider_id) {
        Some(p) if !p.name.is_empty() => p.name.clone(),
        _ if provider_id == LOCAL_PROVIDER => "Local".to_string(),
        _ => provider_id.to_string(),
    }
}

/// The listing JSON shared by the settings listing and the re-list route.
/// The local endpoint falls back to the startup listing: it may be down now,
/// or have loaded another model since start.
fn listing_body(
    app: &App,
    prefs: &Preferences,
    provider_id: &str,
    name: &str,
    base_url: String,
    listed: Result<Vec<ListedModel>, String>,
) -> Value {
    let local = provider_id == LOCAL_PROVIDER;
    let (models, error) = match listed {
        Ok(models) => (models, None),
        Err(e) if local => (app.settings.llm.listed.clone(), Some(e)),
        Err(e) => (Vec::new(), Some(e)),
    };
    let stored = prefs.provider(provider_id).map(|p| &p.models);
    json!({
        "provider": provider_id,
        "name": name,
        "baseUrl": base_url,
        "models": model_entries(&models, &app.settings.llm.defaults, stored, local),
        "error": error,
    })
}

/// The per-model entries of a listing, each carrying its resolved parameters.
/// `stored` is the provider's saved per-model preferences (none when it is not
/// stored yet); `is_local` gates the listed window, which a turn only uses for
/// the local provider (see `Settings::for_turn`).
fn model_entries(
    models: &[ListedModel],
    defaults: &ModelParams,
    stored: Option<&BTreeMap<String, ModelPrefs>>,
    is_local: bool,
) -> Vec<Value> {
    models
        .iter()
        .map(|model| {
            let prefs = stored.and_then(|models| models.get(&model.id));
            let listed = if is_local { model.context_window } else { None };
            json!({
                "id": model.id,
                "contextWindow": model.context_window,
                "resolved": model_view::resolved(defaults, prefs, listed),
            })
        })
        .collect()
}

/// Re-lists one provider's models now. The request carries the baseUrl and
/// API key to use when they are not (yet) stored, so a new provider can be
/// probed before saving. Returns a single listing in the same shape as
/// `GET /api/settings`.
pub fn list_provider(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let body = req.json()?;
    let provider = body
        .get("provider")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "provider must not be empty".to_string())?
        .to_string();
    let base_url = body
        .get("baseUrl")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let api_key: Option<ApiKey> = body
        .get("apiKey")
        .filter(|v| !v.is_null())
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok());

    let prefs = app.store()?.preferences()?;
    let stored = app.settings.endpoint(&prefs, &provider).ok();
    let stored_base_url = stored
        .as_ref()
        .map(|endpoint| endpoint.base_url.clone())
        .unwrap_or_default();
    let base_url = if base_url.is_empty() {
        stored_base_url.clone()
    } else {
        base_url
    };
    // A stored key is only for the stored host: if the request names a
    // different baseUrl, a typo must not hand the secret to another host.
    let api_key = match api_key {
        Some(key) => app.settings.resolve_api_key(key)?,
        None if reuse_stored_key(&base_url, &stored_base_url) => stored.and_then(|e| e.api_key),
        None => None,
    };
    let endpoint = Endpoint {
        base_url: base_url.clone(),
        api_key,
    };
    let listed = list_models(&endpoint);
    let name = provider_name(&prefs, &provider);
    let listing = listing_body(app, &prefs, &provider, &name, base_url, listed);
    send_json(stream, 200, &listing)
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

/// Whether a request may reuse the stored key: only when it does not name a
/// different host, comparing URLs after dropping a trailing slash.
fn reuse_stored_key(requested: &str, stored: &str) -> bool {
    if requested.is_empty() {
        return true;
    }
    requested.trim_end_matches('/') == stored.trim_end_matches('/')
}

#[cfg(test)]
mod tests {
    use super::{model_entries, reuse_stored_key};
    use llm_providers::{ListedModel, ModelParams};

    fn defaults() -> ModelParams {
        ModelParams {
            provider: "local".into(),
            model: "m".into(),
            max_tokens: 8_000,
            context_window: 32_000,
            compact_at: 0.85,
            reasoning_effort: None,
        }
    }

    fn listed() -> Vec<ListedModel> {
        vec![ListedModel {
            id: "m".into(),
            context_window: Some(131_072),
        }]
    }

    #[test]
    fn listed_window_applies_only_to_the_local_provider() {
        // A turn only uses the listed window for the local provider
        // (`Settings::for_turn`); other providers resolve to the default.
        let local = model_entries(&listed(), &defaults(), None, true);
        assert_eq!(local[0]["resolved"]["contextWindow"]["source"], "listed");
        assert_eq!(local[0]["resolved"]["contextWindow"]["value"], 131_072);

        let other = model_entries(&listed(), &defaults(), None, false);
        assert_eq!(other[0]["resolved"]["contextWindow"]["source"], "default");
        assert_eq!(other[0]["resolved"]["contextWindow"]["value"], 32_000);
    }

    #[test]
    fn an_empty_request_url_reuses_the_stored_key() {
        assert!(reuse_stored_key("", "http://stored:1234/v1"));
        assert!(reuse_stored_key("", ""));
    }

    #[test]
    fn the_same_url_with_or_without_a_trailing_slash_reuses_the_stored_key() {
        assert!(reuse_stored_key(
            "http://stored:1234/v1",
            "http://stored:1234/v1/"
        ));
        assert!(reuse_stored_key(
            "http://stored:1234/v1/",
            "http://stored:1234/v1"
        ));
        assert!(reuse_stored_key(
            "http://stored:1234/v1//",
            "http://stored:1234/v1"
        ));
    }

    #[test]
    fn a_different_host_does_not_reuse_the_stored_key() {
        assert!(!reuse_stored_key(
            "http://other:1234/v1",
            "http://stored:1234/v1"
        ));
        assert!(!reuse_stored_key(
            "http://stored:1234/v2",
            "http://stored:1234/v1"
        ));
        assert!(!reuse_stored_key("http://stored:1234/v1", ""));
    }
}
