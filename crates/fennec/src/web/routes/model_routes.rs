//! Model choices for sessions: what the composer offers and what a session runs on.

use crate::agent::ports::EventLog;
use crate::config::Settings;
use crate::domain::event::Event;
use crate::domain::model_choice::{chosen_effort, chosen_model, usable_effort};
use crate::domain::preferences::{
    LOCAL_PROVIDER, ModelPrefs, ModelRef, Preferences, ReasoningEffort,
};
use crate::store::SqliteStore;
use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::{Request, error, send_json};
use serde::Serialize;
use serde_json::{Value, json};
use std::net::TcpStream;

/// Where a session's model comes from, shown next to it.
#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ChoiceSource {
    Session,
    Parent,
    Default,
}

/// The model a session runs on, and where that choice came from. A workflow's
/// child sessions run on the model the human chose for the session.
pub(crate) fn session_choice(
    store: &SqliteStore,
    session_id: &str,
) -> Result<(Option<ModelRef>, ChoiceSource), String> {
    let session = store
        .session(session_id)?
        .ok_or_else(|| format!("no such session {session_id}"))?;
    let events = store.events(session_id)?;
    if let Some(chosen) = chosen_model(&events) {
        return Ok((Some(chosen), ChoiceSource::Session));
    }
    if let Some(parent_id) = session.parent_id {
        let parent_events = store.events(&parent_id)?;
        if let Some(chosen) = chosen_model(&parent_events) {
            return Ok((Some(chosen), ChoiceSource::Parent));
        }
    }
    Ok((None, ChoiceSource::Default))
}

fn prefs_of<'a>(prefs: &'a Preferences, model: &ModelRef) -> Option<&'a ModelPrefs> {
    prefs
        .provider(&model.provider)
        .and_then(|p| p.models.get(&model.model))
}

/// The session's effort (its own choice, else its parent's) and the efforts its
/// model accepts. A choice the model does not accept falls back to auto (`None`).
pub(crate) fn session_effort(
    settings: &Settings,
    store: &SqliteStore,
    session_id: &str,
) -> Result<(Option<ReasoningEffort>, Vec<ReasoningEffort>), String> {
    let session = store
        .session(session_id)?
        .ok_or_else(|| format!("no such session {session_id}"))?;
    let mut chosen = chosen_effort(&store.events(session_id)?);
    if chosen.is_none()
        && let Some(parent_id) = session.parent_id
    {
        chosen = chosen_effort(&store.events(&parent_id)?);
    }
    let (model, _) = session_choice(store, session_id)?;
    let prefs = store.preferences()?;
    let params = settings.llm.for_turn(&prefs, model.as_ref());
    let resolved = ModelRef {
        provider: params.provider.clone(),
        model: params.model.clone(),
    };
    let accepted = ModelPrefs::accepted_efforts(prefs_of(&prefs, &resolved));
    Ok((usable_effort(chosen, &accepted), accepted))
}

/// A stored non-empty name, else "Local" for the local provider, else the id.
fn provider_name(prefs: &Preferences, provider: &str) -> String {
    match prefs.provider(provider) {
        Some(p) if !p.name.is_empty() => p.name.clone(),
        _ if provider == LOCAL_PROVIDER => "Local".to_string(),
        _ => provider.to_string(),
    }
}

fn choice_value(prefs: &Preferences, model: &ModelRef) -> Value {
    json!({
        "provider": model.provider,
        "model": model.model,
        "providerName": provider_name(prefs, &model.provider),
        "acceptedEfforts": ModelPrefs::accepted_efforts(prefs_of(prefs, model)),
    })
}

/// `GET /api/models/choices`: the composer's default and favorites, from
/// stored preferences only.
pub(crate) fn choices(app: &App, stream: &mut TcpStream) -> Result<(), HttpError> {
    let store = app.store()?;
    let prefs = store.preferences()?;
    let default = prefs.default_model(&app.settings.llm.defaults);
    let favorites: Vec<Value> = prefs
        .favorites
        .iter()
        .map(|m| choice_value(&prefs, m))
        .collect();
    send_json(
        stream,
        200,
        &json!({ "default": choice_value(&prefs, &default), "favorites": favorites }),
    )
}

/// `POST /api/sessions/{id}/model`: record the human's model choice.
pub(crate) fn set_model(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), HttpError> {
    let body = req.json()?;
    let provider = body["provider"].as_str().unwrap_or("").to_string();
    let model_id = body["model"].as_str().unwrap_or("").to_string();
    let store = app.store()?;
    let (_, _) = app.require_session(id)?;
    if model_id.is_empty() {
        return error(stream, 400, "model is empty");
    }
    let prefs = store.preferences()?;
    if app.settings.endpoint(&prefs, &provider).is_err() {
        return error(stream, 400, "unknown provider");
    }
    if crate::web::routes::workflow_routes::waiting_for(app, id)?.is_some() {
        return error(stream, 409, "a workflow is running in this session");
    }
    crate::web::routes::session_routes::append_human_event(
        app,
        &store,
        id,
        Some(Event::ModelChanged {
            model: ModelRef {
                provider,
                model: model_id,
            },
        }),
    )?;
    let model = session_model(app, &store, id)?;
    send_json(stream, 200, &json!({ "model": model }))
}

/// `POST /api/sessions/{id}/effort`: record the human's effort choice.
pub(crate) fn set_effort(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), HttpError> {
    let body = req.json()?;
    // Untrusted input: anything but a known effort or null is refused.
    let Ok(effort) = serde_json::from_value::<Option<ReasoningEffort>>(body["effort"].clone())
    else {
        return error(stream, 400, "unknown effort");
    };
    let store = app.store()?;
    let (_, _) = app.require_session(id)?;
    if crate::web::routes::workflow_routes::waiting_for(app, id)?.is_some() {
        return error(stream, 409, "a workflow is running in this session");
    }
    crate::web::routes::session_routes::append_human_event(
        app,
        &store,
        id,
        Some(Event::EffortChanged { effort }),
    )?;
    let model = session_model(app, &store, id)?;
    send_json(stream, 200, &json!({ "model": model }))
}

/// The session's current model as JSON, for the composer and session view.
pub(crate) fn session_model(
    app: &App,
    store: &SqliteStore,
    session_id: &str,
) -> Result<Value, String> {
    let (chosen, source) = session_choice(store, session_id)?;
    let prefs = store.preferences()?;
    let params = app.settings.llm.for_turn(&prefs, chosen.as_ref());
    let (effort, accepted) = session_effort(&app.settings, store, session_id)?;
    Ok(json!({
        "provider": params.provider,
        "model": params.model,
        "providerName": provider_name(&prefs, &params.provider),
        "contextWindow": params.context_window,
        "compactAt": params.compact_at,
        "source": source,
        "effort": effort,
        "acceptedEfforts": accepted,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::preferences::Provider;
    use std::collections::BTreeMap;

    fn prefs_with_provider(id: &str, name: &str) -> Preferences {
        let mut prefs = Preferences::default();
        prefs.providers.push(Provider {
            id: id.into(),
            name: name.into(),
            base_url: String::new(),
            api_key: None,
            models: BTreeMap::new(),
        });
        prefs
    }

    #[test]
    fn provider_name_uses_the_stored_name_when_it_is_set() {
        let prefs = prefs_with_provider("other", "Named");
        assert_eq!(provider_name(&prefs, "other"), "Named");
    }

    #[test]
    fn provider_name_falls_back_to_local_for_the_local_provider() {
        let prefs = Preferences::default();
        assert_eq!(provider_name(&prefs, LOCAL_PROVIDER), "Local");
    }

    #[test]
    fn provider_name_falls_back_to_the_id_for_other_providers() {
        let prefs = Preferences::default();
        assert_eq!(provider_name(&prefs, "other"), "other");
    }
}
