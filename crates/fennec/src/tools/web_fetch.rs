//! Port of openfox `tools/web-fetch.ts`, with deliberate divergences:
//! - the page is saved as a file in a cache and the model gets a short,
//!   deterministic pointer (path, size, outline, first lines), not the page:
//!   it reads the parts it needs with `read_file` or `rg`;
//! - no `format` or `timeout` argument: always Markdown, fixed 30 s timeout;
//! - private, loopback and link-local addresses are refused (SSRF), redirects
//!   are followed by hand so each hop is checked.
//!
//! Not ported: PDF, images, the cf-mitigated retry.

mod address;
mod cache;
mod markdown;
mod outline;

pub(crate) use cache::default_dir as default_cache_dir;

use super::args;
use crate::domain::event::ToolResult;
use serde_json::{Value, json};
use std::net::IpAddr;
use std::path::Path;
use std::time::Duration;

const MAX_RESPONSE_SIZE: u64 = 5 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: usize = 5;
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";
const ACCEPT: &str =
    "text/markdown;q=1.0, text/x-markdown;q=0.9, text/plain;q=0.8, text/html;q=0.7, */*;q=0.1";

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "web_fetch",
            "description": "Fetch a web page and save it as a Markdown file. Returns the file's path, size, an outline of its headings with line numbers and its first lines, not the page: read the parts you need with read_file (offset, limit) or search it with rg.",
            "parameters": {
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch content from (must start with http:// or https://)"
                    }
                },
                "required": ["url"]
            }
        }
    })
}

pub(super) fn run(args: &Value, cache_dir: &Path) -> ToolResult {
    let url = match args::string(args, "url") {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };
    fetch(url, cache_dir, address::is_public)
}

/// `allow` decides which addresses may be contacted: tests widen it to reach
/// a local server.
fn fetch(url: &str, cache_dir: &Path, allow: fn(IpAddr) -> bool) -> ToolResult {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return ToolResult::failure("URL must start with http:// or https://".into());
    }
    cache::purge(cache_dir);
    let path = cache::path_for(cache_dir, url);

    let (markdown, state) = if cache::is_fresh(&path)
        && let Ok(markdown) = std::fs::read_to_string(&path)
    {
        (markdown, "cached")
    } else {
        let markdown = match download(url, allow) {
            Ok(value) => value,
            Err(error) => return ToolResult::failure(error),
        };
        if let Err(error) =
            std::fs::create_dir_all(cache_dir).and_then(|_| std::fs::write(&path, &markdown))
        {
            return ToolResult::failure(format!("Cannot save the page in the cache: {error}"));
        }
        (markdown, "fetched")
    };
    ToolResult::success(pointer(url, &path, state, &markdown), false)
}

fn pointer(url: &str, path: &Path, state: &str, markdown: &str) -> String {
    format!(
        "Saved {url} to {} ({state}, {} KB, {} lines).\nRead the parts you need with read_file (offset, limit) or rg; do not read it whole.\n\n{}\n\nFirst lines:\n{}",
        path.display(),
        markdown.len().div_ceil(1024),
        markdown.lines().count(),
        outline::outline(markdown),
        outline::first_lines(markdown),
    )
}

/// Fetches the page, following redirects by hand, and returns it as Markdown.
fn download(start: &str, allow: fn(IpAddr) -> bool) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .max_redirects(0)
        .max_redirects_will_error(false)
        .build()
        .into();

    let mut url = start.to_string();
    for _ in 0..=MAX_REDIRECTS {
        // ponytail: DNS can change between this check and ureq's own
        // resolution (rebinding); upgrade path: connect to the checked address.
        address::check(&url, allow)?;
        let response = agent
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", ACCEPT)
            .header("Accept-Language", "en-US,en;q=0.9")
            .call()
            .map_err(|error| match error {
                ureq::Error::Timeout(_) => "Request timed out (30s).".to_string(),
                error => error.to_string(),
            })?;
        let status = response.status();
        if status.is_redirection() {
            let location = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| format!("Redirect ({status}) without a Location header"))?;
            url = address::join(&url, location)?;
            continue;
        }
        if !status.is_success() {
            return Err(format!(
                "Request failed with status code: {}",
                status.as_u16()
            ));
        }
        return read_page(response);
    }
    Err(format!("Too many redirects (more than {MAX_REDIRECTS})"))
}

fn read_page(response: ureq::http::Response<ureq::Body>) -> Result<String, String> {
    let too_large = || "Response too large (exceeds 5MB limit)".to_string();
    let declared = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    if declared.is_some_and(|size| size > MAX_RESPONSE_SIZE) {
        return Err(too_large());
    }
    let mime = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if !is_text(&mime) {
        return Err(format!(
            "web_fetch reads text pages; {mime} is not supported yet."
        ));
    }
    let bytes = response
        .into_body()
        .with_config()
        .limit(MAX_RESPONSE_SIZE)
        .read_to_vec()
        .map_err(|error| match error {
            ureq::Error::BodyExceedsLimit(_) => too_large(),
            error => error.to_string(),
        })?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(if mime == "text/html" || mime == "application/xhtml+xml" {
        markdown::to_markdown(&text)
    } else {
        text.into_owned()
    })
}

fn is_text(mime: &str) -> bool {
    mime.is_empty()
        || mime.starts_with("text/")
        || ["json", "xml", "yaml", "javascript", "markdown"]
            .iter()
            .any(|kind| mime.contains(kind))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::test_support::temp_dir;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn anywhere(_: IpAddr) -> bool {
        true
    }

    /// Serves each response in turn on a local port; returns the base URL.
    fn serve(responses: Vec<String>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
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
                stream.write_all(response.as_bytes()).unwrap();
                requests.push(request);
            }
            requests
        });
        (base, handle)
    }

    fn ok(content_type: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[test]
    fn definition_has_the_web_fetch_name_and_required_url() {
        let definition = definition();
        let function = definition.get("function").unwrap();
        assert_eq!(function.get("name").unwrap().as_str(), Some("web_fetch"));
        let parameters = function.get("parameters").unwrap();
        assert_eq!(parameters.get("required").unwrap(), &json!(["url"]));
    }

    #[test]
    fn a_non_http_url_is_refused() {
        let result = run(&json!({ "url": "ftp://example.com/x" }), &temp_dir());
        assert_eq!(
            result.error.as_deref(),
            Some("URL must start with http:// or https://")
        );
        let result = run(&json!({}), &temp_dir());
        assert!(!result.success);
    }

    #[test]
    fn private_addresses_are_refused_before_any_request() {
        for url in [
            "http://127.0.0.1/",
            "http://[::1]/",
            "http://10.0.0.1/",
            "http://192.168.1.1/",
            "http://169.254.169.254/latest/meta-data/",
            "http://localhost/",
        ] {
            let result = run(&json!({ "url": url }), &temp_dir());
            assert!(!result.success, "{url}");
            assert!(result.error.unwrap().starts_with("Refused"), "{url}");
        }
    }

    #[test]
    fn html_is_saved_as_markdown_and_described_by_a_pointer() {
        let (base, server) = serve(vec![ok(
            "text/html; charset=utf-8",
            "<h1>Env</h1><p>one</p><h2>Vars</h2><script>x</script><p>two &amp; three</p>",
        )]);
        let dir = temp_dir();

        let result = fetch(&format!("{base}/doc"), &dir, anywhere);
        let requests = server.join().unwrap();

        assert!(result.success, "{:?}", result.error);
        let output = result.output.unwrap();
        assert!(output.contains("fetched, 1 KB,"));
        assert!(output.contains("Outline (line: heading):\n1: # Env\n5: ## Vars"));
        assert!(output.contains("First lines:\n# Env"));
        let saved = cache::path_for(&dir, &format!("{base}/doc"));
        assert_eq!(
            std::fs::read_to_string(saved).unwrap(),
            "# Env\n\none\n\n## Vars\n\ntwo & three\n"
        );
        let request = requests[0].to_lowercase();
        assert!(request.contains("user-agent: mozilla/5.0"));
        assert!(request.contains("accept: text/markdown;q=1.0"));
    }

    #[test]
    fn markdown_is_saved_as_is_and_a_fresh_cache_skips_the_network() {
        let (base, server) = serve(vec![ok("text/markdown", "plain # text\n")]);
        let dir = temp_dir();
        let url = format!("{base}/a.md");

        let first = fetch(&url, &dir, anywhere);
        server.join().unwrap();
        // The server is gone: only the cache can answer.
        let second = fetch(&url, &dir, anywhere);

        assert!(first.output.unwrap().contains("fetched"));
        let output = second.output.unwrap();
        assert!(output.contains("cached"));
        assert!(output.contains("Outline: none"));
    }

    #[test]
    fn a_redirect_is_followed_and_each_hop_is_checked() {
        let (base, server) = serve(vec![
            "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_string(),
            ok("text/plain", "arrived"),
        ]);
        let result = fetch(&format!("{base}/start"), &temp_dir(), anywhere);
        let requests = server.join().unwrap();
        assert!(result.success, "{:?}", result.error);
        assert!(requests[1].starts_with("GET /final "));

        // Through the real guard, a hop to a local address is refused.
        let (base, server) = serve(vec![
            "HTTP/1.1 302 Found\r\nLocation: http://169.254.169.254/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_string(),
        ]);
        fn only_loopback(ip: IpAddr) -> bool {
            ip.is_loopback() && ip.is_ipv4() || address::is_public(ip)
        }
        let result = fetch(&format!("{base}/x"), &temp_dir(), only_loopback);
        server.join().unwrap();
        assert!(result.error.unwrap().starts_with("Refused"));
    }

    #[test]
    fn errors_and_unsupported_types_are_failures() {
        let (base, server) = serve(vec![
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
        ]);
        let result = fetch(&base, &temp_dir(), anywhere);
        server.join().unwrap();
        assert_eq!(
            result.error.as_deref(),
            Some("Request failed with status code: 404")
        );

        let (base, server) = serve(vec![ok("application/pdf", "%PDF")]);
        let result = fetch(&base, &temp_dir(), anywhere);
        server.join().unwrap();
        assert_eq!(
            result.error.as_deref(),
            Some("web_fetch reads text pages; application/pdf is not supported yet.")
        );
    }

    #[test]
    fn read_file_reads_the_cache_dir_by_absolute_path() {
        let workdir = temp_dir();
        let cache = temp_dir();
        let file = cache.join("page.md");
        std::fs::write(&file, "# Saved\n").unwrap();

        let path = file.display().to_string();
        let result = super::super::read_file::run(
            &json!({ "path": path }),
            &workdir,
            &cache,
            &super::super::path_guard::Outside::none(),
        );
        assert!(result.success, "{:?}", result.error);
        assert!(result.output.unwrap().contains("# Saved"));

        let result = super::super::read_file::run(
            &json!({ "path": path }),
            &workdir,
            &temp_dir(),
            &super::super::path_guard::Outside::none(),
        );
        assert!(!result.success);
    }
}
