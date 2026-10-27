//! Port of openfox `tools/web-search.ts`: SearXNG only.
//!
//! Not ported: Tavily, the engine choice, the SEARXNG_* environment variables
//! (the Settings page holds the instance), web_fetch.

use super::args;
use crate::domain::event::ToolResult;
use crate::domain::preferences::SearchPrefs;
use serde_json::{Value, json};
use std::time::Duration;

const DEFAULT_MAX_RESULTS: i64 = 5;
const TIMEOUT: Duration = Duration::from_secs(15);

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "web_search",
            "description": "Search the web with the SearXNG instance set in Settings. Returns a list of results with title, URL and content snippet for each.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    },
                    "max_results": {
                        "type": "number",
                        "description": "Maximum number of search results to return (default: 5)"
                    }
                },
                "required": ["query"]
            }
        }
    })
}

pub(super) fn run(args: &Value, search: &SearchPrefs) -> ToolResult {
    let query = match args::string(args, "query") {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };

    let max_results = match args::number(args, "max_results", DEFAULT_MAX_RESULTS) {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };
    let max = clamp_max_results(max_results);

    let url =
        match search
            .searxng_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(value) => value,
            None => return ToolResult::failure(
                "web_search requires a SearXNG instance. Set its URL in Settings > Recherche web."
                    .into(),
            ),
        };

    let endpoint = format!("{}/search", url.trim_end_matches('/'));
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into();

    let mut request = agent
        .get(&endpoint)
        .query("format", "json")
        .query("q", query);
    if let Some(key) = search
        .searxng_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        request = request.header("Authorization", &format!("Bearer {key}"));
    }

    let response = match request.call() {
        Ok(response) => response,
        Err(ureq::Error::Timeout(_)) => {
            return ToolResult::failure(
                "Search request timed out (15s). Try a more specific query.".into(),
            );
        }
        Err(error) => return ToolResult::failure(error.to_string()),
    };

    let status = response.status();
    if !status.is_success() {
        let body = response.into_body().read_to_string().unwrap_or_default();
        return ToolResult::failure(format!("SearXNG search failed ({status}): {body}"));
    }

    let body: Value = match response.into_body().read_json() {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error.to_string()),
    };

    let results = parse_results(&body, max);
    if results.is_empty() {
        return ToolResult::success("No search results found.".into(), false);
    }

    ToolResult::success(format_results(&results), false)
}

fn clamp_max_results(value: i64) -> usize {
    if value > 0 {
        value as usize
    } else {
        DEFAULT_MAX_RESULTS as usize
    }
}

struct SearchResult {
    title: String,
    url: String,
    content: String,
}

fn parse_results(body: &Value, max: usize) -> Vec<SearchResult> {
    let field = |item: &Value, name: &str| {
        item.get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    body.get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(max)
        .map(|item| SearchResult {
            title: field(item, "title"),
            url: field(item, "url"),
            content: field(item, "content"),
        })
        .collect()
}

fn format_results(results: &[SearchResult]) -> String {
    results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            format!(
                "[{}] {}\n    URL: {}\n    {}",
                index + 1,
                result.title,
                result.url,
                result.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::preferences::SearchPrefs;
    use serde_json::json;
    use std::io::{Read, Write};

    #[test]
    fn definition_has_the_web_search_name_and_required_query() {
        let definition = definition();
        let function = definition.get("function").unwrap();
        assert_eq!(function.get("name").unwrap().as_str(), Some("web_search"));

        let parameters = function.get("parameters").unwrap();
        assert_eq!(parameters.get("required").unwrap(), &json!(["query"]));
        assert!(
            parameters
                .get("properties")
                .unwrap()
                .get("max_results")
                .is_some()
        );
    }

    #[test]
    fn no_url_is_a_failure() {
        let result = run(&json!({ "query": "rust" }), &SearchPrefs::default());

        assert!(!result.success);
        assert_eq!(
            result.error.as_deref(),
            Some(
                "web_search requires a SearXNG instance. Set its URL in Settings > Recherche web."
            )
        );
    }

    #[test]
    fn blank_url_is_a_failure() {
        let prefs = SearchPrefs {
            searxng_url: Some("   ".into()),
            searxng_api_key: None,
        };
        let result = run(&json!({ "query": "rust" }), &prefs);

        assert!(!result.success);
        assert_eq!(
            result.error.as_deref(),
            Some(
                "web_search requires a SearXNG instance. Set its URL in Settings > Recherche web."
            )
        );
    }

    #[test]
    fn format_results_matches_openfox() {
        let results = vec![
            SearchResult {
                title: "A".into(),
                url: "https://a".into(),
                content: "ca".into(),
            },
            SearchResult {
                title: "B".into(),
                url: "https://b".into(),
                content: "cb".into(),
            },
        ];

        assert_eq!(
            format_results(&results),
            "[1] A\n    URL: https://a\n    ca\n\n[2] B\n    URL: https://b\n    cb"
        );
    }

    #[test]
    fn parse_results_respects_max_and_tolerates_missing_fields() {
        let body = json!({
            "results": [
                { "title": "A", "url": "https://a", "content": "ca" },
                { "url": "https://b" },
                { "content": "cb" }
            ]
        });

        let parsed = parse_results(&body, 2);

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].title, "A");
        assert_eq!(parsed[1].title, "");
        assert_eq!(parsed[1].url, "https://b");
        assert_eq!(parsed[1].content, "");
    }

    #[test]
    fn max_results_at_or_below_zero_uses_the_default() {
        assert_eq!(clamp_max_results(0), 5);
        assert_eq!(clamp_max_results(-3), 5);
        assert_eq!(clamp_max_results(7), 7);
    }

    #[test]
    fn end_to_end_serves_searxng_results() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = String::new();
            let mut buffer = [0u8; 1024];

            while !request.contains("\r\n\r\n") {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                request.push_str(&String::from_utf8_lossy(&buffer[..read]));
            }

            let body = r#"{"results":[{"title":"First","url":"https://first","content":"one"},{"title":"Second","url":"https://second","content":"two"}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
            request
        });

        let prefs = SearchPrefs {
            searxng_url: Some(format!("http://{addr}")),
            searxng_api_key: Some("k".into()),
        };
        let result = run(&json!({ "query": "rust lang" }), &prefs);
        let request = server.join().unwrap();

        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("First"));
        assert!(output.contains("Second"));

        let request_line = request.lines().next().unwrap();
        assert!(request_line.starts_with("GET /search?"));
        assert!(request_line.contains("format=json"));
        assert!(request_line.contains("q=rust+lang") || request_line.contains("q=rust%20lang"));
        assert!(request.to_lowercase().contains("authorization: bearer k"));
    }
}
