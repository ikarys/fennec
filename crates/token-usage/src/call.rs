use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub completion_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Call {
    /// Idempotency key: importing the same external_id twice stores it once. None = always insert.
    #[serde(default)]
    pub external_id: Option<String>,
    pub at_ms: i64,
    pub product: String,
    pub group_id: String,
    pub group_name: String,
    pub run_id: String,
    pub kind: String,
    pub model: String,
    pub usage: Usage,
    pub total_secs: f64,
    pub prefill_speed: f64,
    pub generation_speed: f64,
    #[serde(default)]
    pub breakdown: BTreeMap<String, u64>,
    #[serde(default)]
    pub savings: BTreeMap<String, u64>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_uses_camel_case_and_defaults() {
        let json = r#"{
            "atMs": 1,
            "product": "p",
            "groupId": "g",
            "groupName": "G",
            "runId": "r",
            "kind": "k",
            "model": "m",
            "usage": {"promptTokens":1,"cachedTokens":0,"completionTokens":2},
            "totalSecs":1.0,
            "prefillSpeed":1.0,
            "generationSpeed":1.0
        }"#;

        let call: Call = serde_json::from_str(json).unwrap();

        assert_eq!(call.external_id, None);
        assert!(call.breakdown.is_empty());
        assert!(call.savings.is_empty());
        assert!(call.labels.is_empty());

        let out = serde_json::to_string(&call).unwrap();
        assert!(out.contains("\"atMs\""));
        assert!(out.contains("\"promptTokens\""));
    }
}
