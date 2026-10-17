//! `fennec` — serves the local web UI a developer leads their projects from.
//!
//! The HTTP server is std only: one local user, a handful of routes, no reason
//! to pull an async runtime into a synchronous codebase.

mod state;
mod work;

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

use fennec_os::provider::{self, Settings};
use fennec_os::team;
use fennec_os::world::Worlds;
use serde_json::{Value, json};

use state::Store;

const INDEX: &str = include_str!("../ui/index.html");
const APP: &str = include_str!("../ui/app.js");
/// A request body larger than this is refused: the UI never sends one.
const MAX_BODY: usize = 64 * 1024;

struct Request {
    method: String,
    path: String,
    host: String,
    json: bool,
    body: Vec<u8>,
}

fn main() {
    let addr = std::env::var("FENNEC_LEAD_ADDR").unwrap_or_else(|_| "127.0.0.1:7777".into());
    if let Err(e) = serve(&addr) {
        eprintln!("fennec: {e}");
        std::process::exit(1);
    }
}

fn serve(addr: &str) -> io::Result<()> {
    work::recover(&Store::from_env()?)?;
    // Requests from before stats were kept get theirs, once, off the way.
    std::thread::spawn(|| Store::from_env().map(|s| work::overview(&s)));
    let listener = TcpListener::bind(addr)?;
    let port = listener.local_addr()?.port();
    println!("fennec : http://127.0.0.1:{port}");
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        // ponytail: one thread per connection, fine for one local user.
        std::thread::spawn(move || {
            if let Err(e) = handle(stream, port) {
                eprintln!("fennec: {e}");
            }
        });
    }
    Ok(())
}

fn read_request(stream: &TcpStream) -> io::Result<Request> {
    let mut reader = BufReader::new(stream.take(MAX_BODY as u64 + 16 * 1024));
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let (mut host, mut json, mut length) = (String::new(), false, 0usize);
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "host" => host = value.to_string(),
            "content-type" => json = value.starts_with("application/json"),
            "content-length" => length = value.parse().unwrap_or(usize::MAX),
            _ => {}
        }
    }
    if length > MAX_BODY {
        return Err(io::Error::other("body too large"));
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(Request {
        method,
        path,
        host,
        json,
        body,
    })
}

/// Only the browser on this machine, through this port, may talk to us: the
/// Host check stops DNS rebinding, and requiring a JSON content type forces a
/// CORS preflight, which we never answer, on any cross-site POST.
fn trusted(req: &Request, port: u16) -> bool {
    let host_ok = [format!("127.0.0.1:{port}"), format!("localhost:{port}")].contains(&req.host);
    host_ok && (req.method == "GET" || req.json)
}

fn handle(mut stream: TcpStream, port: u16) -> io::Result<()> {
    let req = read_request(&stream)?;
    if !trusted(&req, port) {
        return respond(&mut stream, "403 Forbidden", "text/plain", b"forbidden");
    }
    let (status, kind, body) = match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") => (
            "200 OK",
            "text/html; charset=utf-8",
            INDEX.as_bytes().to_vec(),
        ),
        ("GET", "/app.js") => (
            "200 OK",
            "text/javascript; charset=utf-8",
            APP.as_bytes().to_vec(),
        ),
        ("GET", path) if path.starts_with("/shots/") => match shot(&path[7..]) {
            Ok(png) => ("200 OK", "image/png", png),
            Err(_) => ("404 Not Found", "text/plain", b"not found".to_vec()),
        },
        (method, path) if path.starts_with("/api/") => {
            let (status, value) = match api(method, path, &req.body) {
                Ok(v) => ("200 OK", v),
                Err(e) => ("400 Bad Request", json!({"error": e.to_string()})),
            };
            (status, "application/json", value.to_string().into_bytes())
        }
        _ => ("404 Not Found", "text/plain", b"not found".to_vec()),
    };
    respond(&mut stream, status, kind, &body)
}

/// A proof screenshot. The name comes from the page: `<hex>-<digit>.png`
/// only, so it cannot leave the proofs directory.
fn shot(file: &str) -> io::Result<Vec<u8>> {
    let valid = file
        .strip_suffix(".png")
        .map(|f| f.strip_suffix("-before").unwrap_or(f))
        .and_then(|f| f.split_once('-'))
        .is_some_and(|(id, n)| {
            !id.is_empty()
                && id.bytes().all(|b| b.is_ascii_hexdigit())
                && n.len() == 1
                && n.as_bytes()[0].is_ascii_digit()
        });
    if !valid {
        return Err(io::Error::other("bad name"));
    }
    std::fs::read(Worlds::from_env()?.root.join("proofs").join(file))
}

fn api(method: &str, path: &str, body: &[u8]) -> io::Result<Value> {
    let store = Store::from_env()?;
    let input = || -> io::Result<Value> { serde_json::from_slice(body).map_err(io::Error::other) };
    let field = |v: &Value, k: &str| v[k].as_str().map(str::to_string).unwrap_or_default();
    match (method, path) {
        ("GET", "/api/view") => store.view(),
        ("POST", "/api/requests") => {
            let v = input()?;
            let id = store.add_request(&field(&v, "project"), &field(&v, "text"))?;
            work::talk(id.clone());
            Ok(json!({"id": id}))
        }
        ("POST", "/api/answer") => {
            let v = input()?;
            let accept = v["accept"].as_bool().unwrap_or(false);
            work::answer(&store, &field(&v, "id"), accept, &field(&v, "correction"))?;
            Ok(json!({}))
        }
        ("POST", "/api/reply") => {
            let v = input()?;
            work::reply(&store, &field(&v, "id"), &field(&v, "text"))?;
            Ok(json!({}))
        }
        ("POST", "/api/land") => {
            work::land(&store, &field(&input()?, "id"))?;
            Ok(json!({}))
        }
        ("POST", "/api/say") => {
            let v = input()?;
            work::say(&store, &field(&v, "id"), &field(&v, "text"))?;
            Ok(json!({}))
        }
        ("POST", "/api/frame") => {
            work::frame_anyway(&store, &field(&input()?, "id"))?;
            Ok(json!({}))
        }
        ("POST", "/api/pass") => {
            work::pass(&store, &field(&input()?, "id"))?;
            Ok(json!({}))
        }
        ("POST", "/api/amend") => {
            let input = input()?;
            work::amend(&store, &field(&input, "id"), &field(&input, "text"))?;
            Ok(json!({}))
        }
        ("POST", "/api/resume") => {
            work::resume(&store, &field(&input()?, "id"))?;
            Ok(json!({}))
        }
        ("POST", "/api/open") => work::open(&store, &field(&input()?, "id")),
        ("POST", "/api/changes") => work::changes(&store, &field(&input()?, "id")),
        ("POST", "/api/journal") => work::journal(&store, &field(&input()?, "id")),
        ("GET", "/api/stats") => work::overview(&store),
        ("POST", "/api/retry") => {
            work::retry(&store, &field(&input()?, "id"))?;
            Ok(json!({}))
        }
        ("POST", "/api/rule") => {
            let v = input()?;
            work::rule(&store, &field(&v, "id"), &field(&v, "scope"))?;
            Ok(json!({}))
        }
        ("POST", "/api/drop") => {
            work::drop(&store, &field(&input()?, "id"))?;
            Ok(json!({}))
        }
        ("POST", "/api/projects") => {
            let input = input()?;
            store.add_project(&field(&input, "path"), &field(&input, "place"))?;
            Ok(json!({}))
        }
        ("POST", "/api/folders") => folders(&field(&input()?, "path")),
        // The key never goes back to the page: only whether one is saved.
        ("POST", "/api/settings") => {
            let s = Settings::saved()?;
            let env: Vec<&str> = provider::ENV
                .into_iter()
                .filter(|v| std::env::var_os(v).is_some())
                .collect();
            Ok(json!({"base_url": s.base_url, "models": s.models,
                      "has_key": s.api_key.is_some(), "env": env,
                      "max_tokens": s.max_tokens}))
        }
        ("POST", "/api/settings/served") => {
            let v = input()?;
            let key = v["api_key"]
                .as_str()
                .filter(|k| !k.is_empty())
                .map(str::to_string)
                .or(Settings::saved()?.api_key);
            // The page puts ureq's wording in French.
            let models = provider::served(&field(&v, "base_url"), key.as_deref())?;
            Ok(json!({"models": models}))
        }
        ("POST", "/api/settings/save") => {
            let v = input()?;
            let base_url = field(&v, "base_url").trim().to_string();
            if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
                return Err(io::Error::other(
                    "l'adresse doit commencer par http:// ou https://",
                ));
            }
            // ponytail: an empty key field keeps the saved key; no way to
            // remove it from the page yet, edit model.json if needed.
            let api_key = v["api_key"]
                .as_str()
                .filter(|k| !k.is_empty())
                .map(str::to_string)
                .or(Settings::saved()?.api_key);
            let models = v["models"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|m| m.as_str().map(str::to_string))
                .collect();
            let max_tokens = v["max_tokens"].as_u64().ok_or_else(|| {
                io::Error::other("le nombre de tokens doit être un entier positif")
            })?;
            Settings {
                base_url,
                models,
                api_key,
                max_tokens,
            }
            .save()?;
            Ok(json!({}))
        }
        ("POST", "/api/team") => team_view(&project(&store, &field(&input()?, "project"))?),
        ("POST", "/api/team/choose") => {
            let v = input()?;
            let remove: Vec<String> = v["remove"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|k| k.as_str().map(str::to_string))
                .collect();
            let path = project(&store, &field(&v, "project"))?;
            team::choose(&path, &field(&v, "team"), &remove)?;
            team_view(&path)
        }
        ("POST", "/api/team/role") => {
            let v = input()?;
            let path = project(&store, &field(&v, "project"))?;
            let key = v["key"].as_str().filter(|k| !k.is_empty());
            team::save_role(
                &path,
                key,
                &field(&v, "name"),
                &field(&v, "stage"),
                &field(&v, "brief"),
                &v["checks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| c.as_str().map(str::to_string))
                    .collect::<Vec<_>>(),
                &field(&v, "model"),
            )?;
            team_view(&path)
        }
        ("POST", "/api/team/save") => {
            let v = input()?;
            let path = project(&store, &field(&v, "project"))?;
            team::save_team(&path, &field(&v, "name"))?;
            team_view(&path)
        }
        ("POST", "/api/team/add") => {
            let v = input()?;
            let path = project(&store, &field(&v, "project"))?;
            team::add(&path, &field(&v, "role"))?;
            team_view(&path)
        }
        _ => Err(io::Error::other("unknown route")),
    }
}

/// One level of the folder browser used to pick a project in a few clicks:
/// the browser cannot hand a page an absolute path, so the server lists them.
/// Team files are written only into a project the human added: the page
/// names a path, and that path must be one of theirs.
fn project(store: &Store, path: &str) -> io::Result<std::path::PathBuf> {
    if !store.projects()?.iter().any(|p| p == path) {
        return Err(io::Error::other("projet inconnu"));
    }
    Ok(path.into())
}

/// A project's team for the "Équipe" dialog: the teams to pick from, the
/// roles of the one picked (left-out ones flagged), and the catalogue roles
/// not in it yet.
fn team_view(path: &Path) -> io::Result<Value> {
    let setup = team::setup(path)?;
    let role = |r: &team::Role| {
        json!({"key": r.key, "name": r.name, "stage": r.stage, "brief": r.brief,
                                    "checks": r.checks, "model": r.model})
    };
    let roles: Vec<Value> = setup
        .roles
        .iter()
        .map(|r| {
            let mut v = role(r);
            v["out"] = json!(setup.remove.contains(&r.key));
            v
        })
        .collect();
    let catalog: Vec<Value> = team::catalog()?
        .iter()
        .filter(|c| !setup.roles.iter().any(|r| r.key == c.key))
        .map(role)
        .collect();
    Ok(
        json!({"teams": team::teams()?, "team": setup.team, "roles": roles, "catalog": catalog,
              "missing": setup.missing.is_some()}),
    )
}

fn folders(path: &str) -> io::Result<Value> {
    let path = if path.is_empty() {
        std::env::var_os("HOME")
            .map(Into::into)
            .unwrap_or_else(|| "/".into())
    } else {
        std::fs::canonicalize(path)?
    };
    let mut children: Vec<Value> = std::fs::read_dir(&path)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| {
            let p = e.path();
            json!({"path": p.display().to_string(), "name": e.file_name().to_string_lossy(), "repo": state::is_repository(&p)})
        })
        .collect();
    children.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let parent = path.parent().map(Path::display).map(|d| d.to_string());
    Ok(
        json!({"path": path.display().to_string(), "parent": parent, "repo": state::is_repository(&path),
               "empty": state::is_empty(&path)?, "children": children}),
    )
}

fn respond(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}
