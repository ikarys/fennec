//! Who may talk to the server. It drives an agent that runs commands, so only
//! the human's own pages may: `Host` must name this server (DNS rebinding),
//! `Origin`, when a browser sends one, must be its pages or Vite's dev server
//! (CSRF), and a POST must carry JSON, which a cross-site form cannot send
//! without a CORS preflight this server never grants.

use super::http::Request;

/// Vite's dev server proxies `/api` to fennec during `npm run dev`.
const DEV_PORT: u16 = 5173;

pub struct Refusal {
    pub status: u16,
    pub message: &'static str,
}

pub fn check(req: &Request, port: u16) -> Result<(), Refusal> {
    // `port` is this server; `port - 1` is the Vite page when the recipe pairs
    // the API one port above its UI (the servers panel), so that origin is
    // allowed too. `DEV_PORT` covers the no-panel recipe, where Vite stays on
    // its default port.
    let local = |hostport: &str| {
        [port, port.saturating_sub(1), DEV_PORT]
            .iter()
            .any(|p| hostport == format!("127.0.0.1:{p}") || hostport == format!("localhost:{p}"))
    };
    let host_ok = req.header("host").is_some_and(local);
    let origin_ok = req
        .header("origin")
        .is_none_or(|o| o.strip_prefix("http://").is_some_and(local));
    if !(host_ok && origin_ok) {
        return Err(Refusal {
            status: 403,
            message: "forbidden origin",
        });
    }
    let json = req
        .header("content-type")
        .is_some_and(|c| c.starts_with("application/json"));
    if req.method == "POST" && !json {
        return Err(Refusal {
            status: 400,
            message: "content-type must be application/json",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn post(headers: &[(&str, &str)]) -> Request {
        Request {
            method: "POST".into(),
            path: "/api/sessions".into(),
            query: String::new(),
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body: Vec::new(),
        }
    }

    fn status(headers: &[(&str, &str)]) -> Option<u16> {
        check(&post(headers), 7777).err().map(|r| r.status)
    }

    const JSON: (&str, &str) = ("Content-Type", "application/json");

    #[test]
    fn only_local_pages_may_post_json() {
        assert_eq!(status(&[("Host", "127.0.0.1:7777"), JSON]), None);
        assert_eq!(
            status(&[
                ("Host", "localhost:5173"),
                ("Origin", "http://localhost:5173"),
                JSON
            ]),
            None
        );
        // The servers panel: the API is on `port`, the UI page on `port - 1`.
        assert_eq!(
            status(&[
                ("Host", "127.0.0.1:7777"),
                ("Origin", "http://127.0.0.1:7776"),
                JSON
            ]),
            None
        );
        // A non-adjacent foreign port is still refused.
        assert_eq!(
            status(&[
                ("Host", "127.0.0.1:7777"),
                ("Origin", "http://127.0.0.1:7775"),
                JSON
            ]),
            Some(403)
        );
        // DNS rebinding: a foreign name resolving to 127.0.0.1.
        assert_eq!(status(&[("Host", "evil.example:7777"), JSON]), Some(403));
        // CSRF: a foreign page posting to the local server.
        assert_eq!(
            status(&[
                ("Host", "127.0.0.1:7777"),
                ("Origin", "https://evil.example"),
                JSON
            ]),
            Some(403)
        );
        assert_eq!(status(&[JSON]), Some(403));
        // A cross-site form posts text/plain or form data.
        assert_eq!(status(&[("Host", "127.0.0.1:7777")]), Some(400));
    }

    #[test]
    fn a_get_without_content_type_to_a_local_host_is_allowed() {
        let req = Request {
            method: "GET".into(),
            path: "/api/sessions".into(),
            query: String::new(),
            headers: vec![("Host".to_string(), "127.0.0.1:7777".to_string())],
            body: Vec::new(),
        };

        assert!(check(&req, 7777).is_ok());
    }

    #[test]
    fn a_json_content_type_with_charset_is_accepted() {
        assert!(
            status(&[
                ("Host", "127.0.0.1:7777"),
                ("Content-Type", "application/json; charset=utf-8"),
            ])
            .is_none()
        );
    }

    #[test]
    fn a_text_json_content_type_is_refused_with_400() {
        assert_eq!(
            status(&[("Host", "127.0.0.1:7777"), ("Content-Type", "text/json")]),
            Some(400)
        );
    }

    #[test]
    fn an_https_localhost_origin_is_refused_with_403() {
        assert_eq!(
            status(&[
                ("Host", "127.0.0.1:7777"),
                ("Origin", "https://localhost:7777"),
                JSON,
            ]),
            Some(403)
        );
    }

    #[test]
    fn an_http_localhost_origin_with_a_local_host_is_allowed() {
        assert!(
            status(&[
                ("Host", "127.0.0.1:7777"),
                ("Origin", "http://localhost:7777"),
                JSON,
            ])
            .is_none()
        );
    }

    #[test]
    fn a_lowercase_host_header_name_is_accepted() {
        assert!(status(&[("host", "127.0.0.1:7777"), JSON]).is_none());
    }

    #[test]
    fn a_null_origin_is_refused_with_403() {
        assert_eq!(
            status(&[("Host", "127.0.0.1:7777"), ("Origin", "null"), JSON]),
            Some(403)
        );
    }

    #[test]
    fn a_foreign_host_is_checked_before_the_content_type() {
        assert_eq!(status(&[("Host", "example.com:7777")]), Some(403));
    }

    #[test]
    fn the_port_argument_is_honoured_for_the_host_check() {
        let allowed = post(&[("Host", "127.0.0.1:9000"), JSON]);
        assert!(check(&allowed, 9000).is_ok());

        let refused = post(&[("Host", "127.0.0.1:7777"), JSON]);
        assert_eq!(check(&refused, 9000).unwrap_err().status, 403);
    }
}
