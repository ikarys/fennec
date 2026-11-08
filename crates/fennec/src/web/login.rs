//! Remote callers' login: a password-gated session cookie.

use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::{self, Request};
use crate::web::routes;
use ring::rand::SecureRandom;
use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Logins {
    password: Mutex<Option<String>>,
    // ponytail: in-memory sessions, persist them if re-login after a restart annoys
    sessions: Mutex<HashMap<String, Instant>>,
    attempts: Mutex<()>,
}

/// A session lasts as long as its cookie; the server forgets it then too.
const SESSION_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);

impl Logins {
    pub fn new(password: Option<String>) -> Self {
        Self {
            password: Mutex::new(password),
            sessions: Mutex::new(HashMap::new()),
            attempts: Mutex::new(()),
        }
    }

    pub fn enabled(&self) -> bool {
        crate::locks::lock(&self.password).is_some()
    }

    pub fn set_password(&self, hash: String) {
        // A stolen token must not survive a password change.
        crate::locks::lock(&self.sessions).clear();
        *crate::locks::lock(&self.password) = Some(hash);
    }

    pub fn has_session(&self, token: &str) -> bool {
        let mut sessions = crate::locks::lock(&self.sessions);
        let expired = sessions
            .get(token)
            .is_some_and(|&created| Instant::now().duration_since(created) > SESSION_TTL);
        if expired {
            sessions.remove(token);
            false
        } else {
            sessions.contains_key(token)
        }
    }

    pub fn add_session(&self, token: &str) {
        crate::locks::lock(&self.sessions).insert(token.to_string(), Instant::now());
    }

    pub fn verify_password(&self, candidate: &str) -> bool {
        crate::locks::lock(&self.password)
            .as_deref()
            .is_some_and(|stored| crate::domain::password::verify(stored, candidate))
    }
}

impl Default for Logins {
    fn default() -> Self {
        Self::new(None)
    }
}

/// What a remote caller's request gets, before anything is written.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Access {
    Granted,
    LoginAttempt,
    Refused,
    LoginPage,
}

pub(crate) fn access(logins: &Logins, req: &Request) -> Access {
    if session_cookie(req).is_some_and(|token| logins.has_session(token)) {
        Access::Granted
    } else if req.method == "POST" && req.path == "/api/login" {
        Access::LoginAttempt
    } else if req.path.starts_with("/api/") {
        Access::Refused
    } else {
        Access::LoginPage
    }
}

pub fn gate(app: &Arc<App>, req: &Request, stream: &mut TcpStream) -> Result<(), HttpError> {
    match access(&app.logins, req) {
        Access::Granted => routes::handle(app, req, stream),
        Access::LoginAttempt => login_attempt(app, req, stream),
        Access::Refused => http::send_json(
            stream,
            401,
            &serde_json::json!({ "error": "login required" }),
        ),
        Access::LoginPage => login_page(stream),
    }
}

fn login_attempt(app: &Arc<App>, req: &Request, stream: &mut TcpStream) -> Result<(), HttpError> {
    let _attempts = crate::locks::lock(&app.logins.attempts);
    let candidate = req
        .json()
        .ok()
        .and_then(|value| value.get("password")?.as_str().map(str::to_string))
        .unwrap_or_default();

    if app.logins.verify_password(&candidate) {
        let mut token_bytes = [0u8; 32];
        ring::rand::SystemRandom::new()
            .fill(&mut token_bytes)
            .map_err(|e| e.to_string())?;
        let token = token_bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        app.logins.add_session(&token);

        let cookie = format!(
            "fennec_session={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}",
            SESSION_TTL.as_secs()
        );
        http::send_with(
            stream,
            200,
            "application/json",
            &[("Set-Cookie", &cookie)],
            b"{}",
        )
    } else {
        std::thread::sleep(Duration::from_secs(1));
        http::send_json(
            stream,
            401,
            &serde_json::json!({ "error": "wrong password" }),
        )
    }
}

fn session_cookie(req: &Request) -> Option<&str> {
    req.header("cookie")?.split(';').find_map(|pair| {
        let (name, value) = pair.trim().split_once('=')?;
        (name == "fennec_session").then_some(value)
    })
}

fn login_page(stream: &mut TcpStream) -> Result<(), HttpError> {
    let html = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>fennec</title>
<style>
:root { color-scheme: dark light; }
body { margin: 0; min-height: 100vh; display: grid; place-items: center; background: canvas; color: canvastext; font-family: system-ui, sans-serif; }
form { display: grid; gap: 0.5rem; padding: 2rem; border: 1px solid canvastext; }
input, button { padding: 0.5rem; }
#status { min-height: 1.2em; }
</style>
</head>
<body>
<form>
<h1>fennec</h1>
<input type="password" name="password" placeholder="Password" required>
<button type="submit">Sign in</button>
<p id="status"></p>
</form>
<script>
document.querySelector('form').addEventListener('submit', async function (event) {
  event.preventDefault();
  var password = document.querySelector('input').value;
  var status = document.getElementById('status');
  var response = await fetch('/api/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ password: password })
  });
  if (response.ok) {
    window.location.reload();
  } else {
    var payload = await response.json().catch(function () { return {}; });
    status.textContent = payload.error || 'Login failed';
  }
});
</script>
</body>
</html>
"#;
    http::send(stream, 200, "text/html; charset=utf-8", html.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: &str, path: &str, headers: Vec<(&str, &str)>, body: &[u8]) -> Request {
        Request {
            method: method.to_string(),
            path: path.to_string(),
            query: String::new(),
            headers: headers
                .into_iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
            body: body.to_vec(),
        }
    }

    #[test]
    fn an_api_call_without_a_cookie_is_refused() {
        let logins = Logins::new(None);
        let req = request("GET", "/api/status", Vec::new(), b"");
        assert_eq!(access(&logins, &req), Access::Refused);
    }

    #[test]
    fn a_page_without_a_cookie_gets_the_login_page() {
        let logins = Logins::new(None);
        let req = request("GET", "/", Vec::new(), b"");
        assert_eq!(access(&logins, &req), Access::LoginPage);
    }

    #[test]
    fn a_login_post_is_an_attempt() {
        let logins = Logins::new(None);
        let req = request("POST", "/api/login", Vec::new(), b"");
        assert_eq!(access(&logins, &req), Access::LoginAttempt);
    }

    #[test]
    fn a_valid_session_cookie_is_granted() {
        let logins = Logins::new(None);
        logins.add_session("token123");
        let req = request(
            "GET",
            "/api/status",
            vec![("cookie", "other=1; fennec_session=token123")],
            b"",
        );
        assert_eq!(access(&logins, &req), Access::Granted);
    }

    #[test]
    fn an_unknown_session_cookie_is_refused() {
        let logins = Logins::new(None);
        let req = request(
            "GET",
            "/api/status",
            vec![("cookie", "fennec_session=unknown")],
            b"",
        );
        assert_eq!(access(&logins, &req), Access::Refused);
    }

    #[test]
    fn changing_the_password_ends_every_session() {
        let logins = Logins::new(None);
        logins.add_session("old");
        assert!(logins.has_session("old"));
        let hash = crate::domain::password::hash("new", &[0u8; 8], 1);
        logins.set_password(hash);
        assert!(!logins.has_session("old"));
    }

    #[test]
    fn an_expired_session_is_refused() {
        let logins = Logins::new(None);
        let token = "expired";
        let created = match Instant::now().checked_sub(SESSION_TTL + Duration::from_secs(1)) {
            Some(created) => created,
            None => return,
        };
        crate::locks::lock(&logins.sessions).insert(token.to_string(), created);
        assert!(!logins.has_session(token));
    }

    #[test]
    fn the_right_password_verifies_and_a_wrong_one_does_not() {
        let logins = Logins::new(None);
        let hash = crate::domain::password::hash("right", &[0u8; 8], 1);
        logins.set_password(hash);
        assert!(logins.verify_password("right"));
        assert!(!logins.verify_password("wrong"));
    }
}
