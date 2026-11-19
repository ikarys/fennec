//! The last model listing each endpoint answered: the Settings page shows it at once and
//! re-lists in the background, so a busy model server never delays the page.

use llm_providers_connector::{Endpoint, ListedModel, list_models};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// One endpoint's last listing: a success or a failure (a failure is cached too,
/// so a down endpoint never blocks the page on every call).
type Listing = Result<Vec<ListedModel>, String>;

#[derive(Default, Clone)]
pub struct Listings(Arc<Mutex<HashMap<String, Listing>>>);

impl Listings {
    /// The last listing — a success or a failure — at once, refreshed in a background
    /// thread; the first time (nothing cached) it lists now and waits. Caching a failure
    /// keeps a down or slow endpoint from blocking the page on every call.
    pub fn get(&self, endpoint: &Endpoint) -> Result<Vec<ListedModel>, String> {
        let key = endpoint.base_url.clone();
        let cached = crate::locks::lock(&self.0).get(&key).cloned();
        match cached {
            Some(result) => {
                // ponytail: concurrent page loads may each start a refresh (each bounded by
                // the 5 s listing timeout); dedupe if it ever matters.
                let listings = self.clone();
                let endpoint = endpoint.clone();
                std::thread::spawn(move || {
                    let _ = listings.refresh(&endpoint);
                });
                result
            }
            None => self.refresh(endpoint),
        }
    }

    /// Lists now, and caches the result (the re-list button: the human asked for the truth).
    pub fn refresh(&self, endpoint: &Endpoint) -> Result<Vec<ListedModel>, String> {
        let result = list_models(endpoint);
        let mut cache = crate::locks::lock(&self.0);
        cache.insert(endpoint.base_url.clone(), result.clone());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn a_failed_refresh_is_cached_so_get_does_not_block_again() {
        let listings = Listings::default();
        let endpoint = Endpoint {
            base_url: "http://127.0.0.1:9/v1".into(),
            api_key: None,
        };

        let err = listings.refresh(&endpoint).unwrap_err();
        assert!(err.contains("http://127.0.0.1:9/v1/models"));
        // The failure is cached: a down endpoint no longer blocks every call.
        assert!(!listings.0.lock().unwrap().is_empty());

        let started = Instant::now();
        let again = listings.get(&endpoint).unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(again, err);
    }

    #[test]
    fn get_returns_a_cached_success_immediately() {
        let listings = Listings::default();
        let endpoint = Endpoint {
            base_url: "http://127.0.0.1:9/v1".into(),
            api_key: None,
        };

        listings.0.lock().unwrap().insert(
            endpoint.base_url.clone(),
            Ok(vec![ListedModel {
                id: "cached".into(),
                context_window: Some(100),
            }]),
        );

        let started = Instant::now();
        let models = listings.get(&endpoint).unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(models[0].id, "cached");
    }
}
