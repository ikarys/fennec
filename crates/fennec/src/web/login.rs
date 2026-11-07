//! Remote callers' login: a password-gated session cookie.

use super::App;
use super::http::{self, Request};
use super::routes;
use ring::rand::SecureRandom;
use std::collections::HashSet;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct Logins {
    password: Mutex<Option<String>>,
    // ponytail: in-memory sessions, persist them if re-login after a restart annoys
    sessions: Mutex<HashSet<String>>,
    attempts: Mutex<()>,
}

impl Logins {
    pub fn new(password: Option<String>) -> Self {
        Self {
            password: Mutex::new(password),
            sessions: Mutex::new(HashSet::new()),
            attempts: Mutex::new(()),
        }
    }

    pub fn enabled(&self) -> bool {
        crate::locks::lock(&self.password).is_some()
    }

    pub fn set_password(&self, hash: String) {
        *crate::locks::lock(&self.password) = Some(hash);
    }

    pub fn has_session(&self, token: &str) -> bool {
        crate::locks::lock(&self.sessions).contains(token)
    }

    pub fn add_session(&self, token: &str) {
        crate::locks::lock(&self.sessions).insert(token.to_string());
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

pub fn gate(app: &Arc<App>, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    if session_cookie(req).is_some_and(|token| app.logins.has_session(token)) {
        return routes::handle(app, req, stream);
    }

    if req.method == "POST" && req.path == "/api/login" {
        return login_attempt(app, req, stream);
    }

    if req.path.starts_with("/api/") {
        return http::send_json(
            stream,
            401,
            &serde_json::json!({ "error": "login required" }),
        );
    }

    login_page(stream)
}

fn login_attempt(app: &Arc<App>, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
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

        let cookie =
            format!("fennec_session={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=2592000");
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

fn login_page(stream: &mut TcpStream) -> Result<(), String> {
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
