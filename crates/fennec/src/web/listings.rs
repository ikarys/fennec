//! The last model listing each endpoint answered: the Settings page shows it at once and
//! re-lists in the background, so a busy model server never delays the page.

use llm_providers_connector::{Endpoint, ListedModel, list_models};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Default, Clone)]
pub struct Listings(Arc<Mutex<HashMap<String, Vec<ListedModel>>>>);

impl Listings {
    /// The cached listing at once, refreshed in a background thread; the first time
    /// (nothing cached) it lists now and waits.
    pub fn get(&self, endpoint: &Endpoint) -> Result<Vec<ListedModel>, String> {
        let key = endpoint.base_url.clone();
        let cached = self
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned();
        match cached {
            Some(models) => {
                // ponytail: concurrent page loads may each start a refresh (each bounded by
                // the 5 s listing timeout); dedupe if it ever matters.
                let listings = self.clone();
                let endpoint = endpoint.clone();
                std::thread::spawn(move || {
                    let _ = listings.refresh(&endpoint);
                });
                Ok(models)
            }
            None => self.refresh(endpoint),
        }
    }

    /// Lists now, and caches a success (the re-list button: the human asked for the truth).
    pub fn refresh(&self, endpoint: &Endpoint) -> Result<Vec<ListedModel>, String> {
        let models = list_models(endpoint)?;
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(endpoint.base_url.clone(), models.clone());
        Ok(models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn refresh_failure_caches_nothing_and_get_returns_cached_immediately() {
        let listings = Listings::default();
        let endpoint = Endpoint {
            base_url: "http://127.0.0.1:9/v1".into(),
            api_key: None,
        };

        let err = listings.refresh(&endpoint).unwrap_err();
        assert!(err.contains("http://127.0.0.1:9/v1/models"));
        assert!(listings.0.lock().unwrap().is_empty());

        listings.0.lock().unwrap().insert(
            endpoint.base_url.clone(),
            vec![ListedModel {
                id: "cached".into(),
                context_window: Some(100),
            }],
        );

        let started = Instant::now();
        let models = listings.get(&endpoint).unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(models[0].id, "cached");
    }
}
