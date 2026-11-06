//! The first-run checklist's facts, read from the machine: the default model's endpoint, the sandbox, PATH, the store.

use crate::config::Settings;
use crate::domain::setup::{Facts, ModelFact, SandboxFact};
use crate::store::SqliteStore;
use crate::tools::{SandboxHealth, probe};
use llm_providers_connector::list_models;

pub fn facts(settings: &Settings, store: &SqliteStore) -> Result<Facts, String> {
    let prefs = store.preferences().map_err(|e| e.to_string())?;
    let default = prefs.default_model(&settings.llm.defaults);

    let model = match settings.endpoint(&prefs, &default.provider) {
        Err(error) => ModelFact::Unreachable { error },
        Ok(endpoint) => match list_models(&endpoint) {
            Ok(listed) => ModelFact::Reachable {
                listed: listed.iter().any(|m| m.id == default.model),
                model: default.model.clone(),
                base_url: endpoint.base_url.clone(),
            },
            Err(error) => ModelFact::Unreachable { error },
        },
    };

    let sandbox = if !prefs.sandbox.enabled {
        SandboxFact::Off
    } else {
        match probe() {
            SandboxHealth::Ready => SandboxFact::Ready,
            SandboxHealth::Missing => SandboxFact::Missing,
            SandboxHealth::Blocked(detail) => SandboxFact::Blocked(detail),
        }
    };

    let git = crate::scanners::find("git").is_some();
    let projects = store.projects()?.len();
    let scanners_found = crate::domain::scanner::SCANNERS
        .iter()
        .filter(|scanner| crate::scanners::find(scanner.binary).is_some())
        .count();
    let scanners_known = crate::domain::scanner::SCANNERS.len();

    Ok(Facts {
        model,
        sandbox,
        git,
        projects,
        scanners_found,
        scanners_known,
    })
}
