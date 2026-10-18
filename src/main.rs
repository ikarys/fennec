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
/// A request body larger than this is refused: an attached image, in
/// base64, is the largest the UI sends.
const MAX_BODY: usize = 16 * 1024 * 1024;

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
    fennec_os::chat::recover(&Worlds::from_env()?)?;
    // The old per-project skill choice moves to the global list, once.
    {
        let store = Store::from_env()?;
        let paths: Vec<std::path::PathBuf> =
            store.projects()?.into_iter().map(Into::into).collect();
        let projects: Vec<&Path> = paths.iter().map(|p| p.as_path()).collect();
        fennec_os::skill::migrate(&projects)?;
    }
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
        ("GET", path) if path.starts_with("/images/") => {
            match Store::from_env().and_then(|s| s.image(&path[8..])) {
                Ok(bytes) => {
                    let kind = match path.rsplit('.').next() {
                        Some("jpg") => "image/jpeg",
                        Some("gif") => "image/gif",
                        Some("webp") => "image/webp",
                        _ => "image/png",
                    };
                    ("200 OK", kind, bytes)
                }
                Err(_) => ("404 Not Found", "text/plain", b"not found".to_vec()),
            }
        }
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
        ("POST", "/api/image") => Ok(json!({"line": store.save_image(&field(&input()?, "data"))?})),
        ("POST", "/api/requests") => {
            let v = input()?;
            // Role keys this request does without, among its project's team.
            let off: Vec<String> = v["off"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|k| k.as_str().map(str::to_string))
                .collect();
            let later = v["later"] == true;
            let id = store.add_request(&field(&v, "project"), &field(&v, "text"), &off, later)?;
            if !later {
                work::talk(id.clone());
            }
            Ok(json!({"id": id}))
        }
        // The backlog: a request kept for later, started, reworded or forgotten.
        ("POST", "/api/launch") => {
            let id = field(&input()?, "id");
            store.launch(&id)?;
            work::talk(id);
            Ok(json!({}))
        }
        ("POST", "/api/rewrite") => {
            let v = input()?;
            store.rewrite(&field(&v, "id"), &field(&v, "text"))?;
            Ok(json!({}))
        }
        ("POST", "/api/forget") => {
            store.forget(&field(&input()?, "id"))?;
            Ok(json!({}))
        }
        ("POST", "/api/request/archive") => {
            let v = input()?;
            store.archive_request(&field(&v, "id"), v["on"] == true)?;
            Ok(json!({}))
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
        ("POST", "/api/page") => work::page(&store, &field(&input()?, "id")),
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
        ("POST", "/api/stop") => {
            work::stop(&store, &field(&input()?, "id"))?;
            Ok(json!({}))
        }
        ("POST", "/api/projects") => {
            let input = input()?;
            store.add_project(&field(&input, "path"), &field(&input, "place"))?;
            Ok(json!({}))
        }
        ("POST", "/api/projects/archive") => {
            let v = input()?;
            store.set_project_archived(&field(&v, "path"), v["on"] == true)?;
            Ok(json!({}))
        }
        ("POST", "/api/projects/remove") => {
            store.remove_project(&field(&input()?, "path"))?;
            Ok(json!({}))
        }
        ("POST", "/api/folders") => {
            let v = input()?;
            let (path, name) = (field(&v, "path"), field(&v, "create"));
            if name.is_empty() {
                folders(&path)
            } else {
                folders(&new_folder(&path, &name)?)
            }
        }
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
        ("POST", "/api/skills") => {
            // The machine's skills, the enabled ones first; or first enable
            // one or take it back. A project, if named, only widens the
            // catalogue with its own .claude/skills.
            let v = input()?;
            let p = field(&v, "project");
            let path = if p.is_empty() {
                None
            } else {
                Some(project(&store, &p)?)
            };
            let name = field(&v, "name");
            if !name.is_empty() {
                fennec_os::skill::choose(&name, v["on"] == true)?;
            }
            let chosen: Vec<String> = fennec_os::skill::chosen(path.as_deref())?
                .into_iter()
                .map(|s| s.name)
                .collect();
            let home = std::env::var("HOME").unwrap_or_default();
            let mut all: Vec<Value> = fennec_os::skill::catalog(path.as_deref())?
                .into_iter()
                .map(|s| {
                    let from = s.dir.display().to_string();
                    json!({"name": s.name, "description": s.description,
                           "chosen": chosen.contains(&s.name),
                           "from": from.replacen(&home, "~", 1)})
                })
                .collect();
            all.sort_by_key(|s| s["chosen"] != true);
            Ok(json!(all))
        }
        // Discussions: a plain conversation with an agent in a folder the
        // person chose; the chat file is the exchange, the page follows it.
        ("POST", "/api/chats") => {
            let folder = field(&input()?, "folder");
            let chat = fennec_os::chat::start(&Worlds::from_env()?, Path::new(&folder))?;
            Ok(json!({"id": chat["id"]}))
        }
        ("POST", "/api/chat/archive") => {
            let v = input()?;
            fennec_os::chat::archive(&Worlds::from_env()?, &field(&v, "id"), v["on"] == true)?;
            Ok(json!({}))
        }
        ("POST", "/api/chat/forget") => {
            fennec_os::chat::forget(&Worlds::from_env()?, &field(&input()?, "id"))?;
            Ok(json!({}))
        }
        ("GET", "/api/chats") => chats_list(&store, false),
        ("GET", "/api/chats/archived") => chats_list(&store, true),
        ("POST", "/api/chat") => {
            let chat = fennec_os::chat::load(&Worlds::from_env()?, &field(&input()?, "id"))?;
            Ok(chat_view(&store, &chat))
        }
        ("POST", "/api/chat/say") => {
            let v = input()?;
            let (id, text) = (field(&v, "id"), field(&v, "text"));
            fennec_os::chat::say(&Worlds::from_env()?, &id, &text)?;
            work::chat(id);
            Ok(json!({}))
        }
        ("POST", "/api/chat/stop") => {
            fennec_os::chat::stop(&Worlds::from_env()?, &field(&input()?, "id"))?;
            Ok(json!({}))
        }
        // The person's answer to a question the turn is waiting on: allow
        // opens the folder for the chat, refuse covers the call only.
        ("POST", "/api/chat/decide") => {
            let v = input()?;
            fennec_os::chat::decide(&Worlds::from_env()?, &field(&v, "id"), v["allow"] == true)?;
            Ok(json!({}))
        }
        // A folder opened for the chat, taken back.
        ("POST", "/api/chat/grant") => {
            let v = input()?;
            fennec_os::chat::ungrant(
                &Worlds::from_env()?,
                &field(&v, "id"),
                &field(&v, "directory"),
            )?;
            Ok(json!({}))
        }
        // The exchange becomes a request for the folder's project, kept in the
        // backlog: the human launches it.
        ("POST", "/api/chat/request") => {
            let chat = fennec_os::chat::load(&Worlds::from_env()?, &field(&input()?, "id"))?;
            let folder = chat["folder"].as_str().unwrap_or("");
            let project = store
                .projects()?
                .into_iter()
                .find(|p| p == folder)
                .ok_or_else(|| io::Error::other("ce dossier n'est pas un projet"))?;
            let id = store.add_request(&project, &transcript(&chat)?, &[], true)?;
            Ok(json!({"id": id}))
        }
        ("POST", "/api/rules") => {
            // Read, or first add or take back one rule said on the project page.
            let v = input()?;
            let path = project(&store, &field(&v, "project"))?;
            let (scope, text) = (field(&v, "scope"), field(&v, "text"));
            match field(&v, "change").as_str() {
                "add" => fennec_os::rules::say(&path, &scope, &text)?,
                "remove" => fennec_os::rules::unsay(&path, &scope, &text)?,
                _ => {}
            }
            let said = fennec_os::rules::said(&path)?;
            Ok(json!(
                said.iter()
                    .map(|s| json!({"scope": s.scope, "text": s.text}))
                    .collect::<Vec<_>>()
            ))
        }
        ("POST", "/api/open-project") => {
            let path = project(&store, &field(&input()?, "project"))?;
            Ok(json!({"folder": path, "opened": work::editor(&path)}))
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

/// The discussion list, archived or not: each entry says whether its folder
/// is one of the human's projects, so the page can mark it.
fn chats_list(store: &Store, archived: bool) -> io::Result<Value> {
    let projects = store.projects()?;
    let list = if archived {
        fennec_os::chat::list_archived(&Worlds::from_env()?)?
    } else {
        fennec_os::chat::list(&Worlds::from_env()?)?
    };
    Ok(json!(
        list.into_iter()
            .map(|mut c| {
                c["project"] = json!(
                    projects
                        .iter()
                        .any(|p| p == c["folder"].as_str().unwrap_or(""))
                );
                c
            })
            .collect::<Vec<_>>()
    ))
}

/// What the page reads of a discussion: the chat file, its cost, and
/// whether its folder is one of the human's projects.
fn chat_view(store: &Store, chat: &Value) -> Value {
    let folder = chat["folder"].as_str().unwrap_or("");
    let mut v = chat.clone();
    v["project"] = json!(
        store
            .projects()
            .is_ok_and(|p| p.iter().any(|x| x == folder))
    );
    v["stats"] = json!(work::cost(&chat["turns"]));
    v
}

/// The exchange, in the human's words, as a request: who said what, no tools.
fn transcript(chat: &Value) -> io::Result<String> {
    let mut text = String::new();
    for m in chat["messages"].as_array().cloned().unwrap_or_default() {
        let content = m["content"].as_str().unwrap_or("").trim();
        if content.is_empty() {
            continue;
        }
        let who = match m["role"].as_str() {
            Some("user") => "Toi",
            Some("assistant") => "fennec",
            _ => continue,
        };
        text.push_str(who);
        text.push_str(" : ");
        text.push_str(content);
        text.push('\n');
    }
    if text.trim().is_empty() {
        return Err(io::Error::other("aucun message à transformer en demande"));
    }
    Ok(text)
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

/// A new project starts as a new folder: made here rather than in a terminal.
/// One plain name, inside the folder shown.
fn new_folder(parent: &str, name: &str) -> io::Result<String> {
    let name = name.trim();
    if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\', '\0']) {
        return Err(io::Error::other(
            "un nom de dossier simple, sans / ni point au début",
        ));
    }
    let path = std::fs::canonicalize(parent)?.join(name);
    std::fs::create_dir(&path).map_err(|e| match e.kind() {
        io::ErrorKind::AlreadyExists => io::Error::other(format!("{name} existe déjà ici")),
        _ => e,
    })?;
    Ok(path.display().to_string())
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
    // The home folder is often a git repo (dotfiles), never a project.
    let home = std::env::var_os("HOME").is_some_and(|h| Path::new(&h) == path);
    Ok(
        json!({"path": path.display().to_string(), "parent": parent, "repo": state::is_repository(&path),
               "empty": state::is_empty(&path)?, "home": home, "children": children}),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_exchange_becomes_the_request_that_was_said() {
        let chat = json!({"messages": [
            {"role": "user", "content": "regarde où on en est"},
            {"role": "assistant", "content": "", "tool_calls": [
                {"id": "1", "function": {"name": "shell", "arguments": "{}"}}]},
            {"role": "tool", "tool_call_id": "1", "content": "…"},
            {"role": "assistant", "content": "c'est à jour"},
            {"role": "assistant", "content": ""}
        ]});
        assert_eq!(
            transcript(&chat).unwrap(),
            "Toi : regarde où on en est\nfennec : c'est à jour\n"
        );
    }

    #[test]
    fn a_chat_without_words_keeps_no_request() {
        assert!(transcript(&json!({"messages": []})).is_err());
        assert!(transcript(&json!({"messages": [{"role": "tool", "content": "…"}]})).is_err());
    }

    #[test]
    fn a_chat_costs_what_its_calls_cost() {
        let turns = json!([
            {"model_ms": 400, "usage": {"prompt_tokens": 1000, "completion_tokens": 60,
             "prompt_tokens_details": {"cached_tokens": 500}}},
            {"model_ms": 100, "usage": {"prompt_tokens": 1100, "completion_tokens": 5}}
        ]);
        let s = work::cost(&turns);
        assert_eq!(s["turns"], 2);
        assert_eq!(s["ms"], 500);
        assert_eq!(s["sent"], 2100);
        assert_eq!(s["cached"], 500);
        assert_eq!(s["received"], 65);
    }
}
