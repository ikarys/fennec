//! The endpoint's model listing (`/models`), moved out of `config.rs` so the
//! web API can refresh it without environment settings.

use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedModel {
    pub id: String,
    pub context_window: Option<u64>,
}

/// Lists the models the endpoint serves. `max_model_len` is vLLM's field.
pub fn list_models(base_url: &str, api_key: Option<&str>) -> Result<Vec<ListedModel>, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    // Bounded: fennec asks at startup and on the Settings page, and an
    // endpoint that drops packets would otherwise hang both.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .build()
        .into();
    let mut req = agent.get(&url);
    if let Some(key) = api_key {
        req = req.header("Authorization", &format!("Bearer {key}"));
    }
    let value: serde_json::Value = req
        .call()
        .and_then(|r| r.into_body().read_json())
        .map_err(|e| format!("{url}: {e}"))?;
    let data = value
        .get("data")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(data
        .into_iter()
        .filter_map(|m| {
            let id = m.get("id")?.as_str()?.to_string();
            (!id.is_empty()).then(|| ListedModel {
                id,
                context_window: m.get("max_model_len").and_then(serde_json::Value::as_u64),
            })
        })
        .collect())
}
