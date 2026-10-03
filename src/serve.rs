//! The review surface: a local web page over the same store as the CLI.
//! Specs, runs and know-how waiting on the human, their diffs, and the
//! decisions on them. Compiling, solving and landing take minutes, so every
//! decision runs as a job; a framed job list follows them.
//!
//! The page can land worlds, so it is guarded like a harness operation:
//! loopback only, a per-start secret exchanged for a SameSite=Strict cookie
//! (no other site can drive it), a Host check (no DNS rebinding), and no
//! script at all: a diff is written by an agent, and the CSP keeps it inert.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::provider::{Model, OpenAi};
use crate::world::Worlds;
use crate::{review, sleep, solve, spec};

const STEPS: usize = 40;
const COOKIE: &str = "fennec-os";

struct Job {
    label: String,
    /// `None` while running.
    outcome: Option<Result<String, String>>,
}

struct Server {
    worlds: Worlds,
    token: String,
    hosts: [String; 2],
    jobs: Mutex<Vec<Job>>,
}

struct Request {
    method: String,
    path: String,
    query: String,
    headers: Vec<(String, String)>,
    body: String,
}

/// A secret for one `serve`: whoever opens the printed URL is the human.
pub fn token() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Serves until the process ends, one thread per connection.
pub fn serve(worlds: Worlds, listener: TcpListener, token: &str) -> io::Result<()> {
    let port = listener.local_addr()?.port();
    let server = Arc::new(Server {
        worlds,
        token: token.to_string(),
        hosts: [format!("127.0.0.1:{port}"), format!("localhost:{port}")],
        jobs: Mutex::default(),
    });
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let server = Arc::clone(&server);
        std::thread::spawn(move || {
            let response = match read_request(&stream) {
                Ok(req) => server.respond(&req),
                Err(_) => reply(400, "", "bad request"),
            };
            let _ = stream.write_all(response.as_bytes());
        });
    }
    Ok(())
}

fn read_request(stream: &TcpStream) -> io::Result<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(Read::take(stream, 1 << 20));
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    let len = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0usize);
    let mut body = vec![0; len];
    reader.read_exact(&mut body)?;
    Ok(Request {
        method,
        path: path.to_string(),
        query: query.to_string(),
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

impl Server {
    fn respond(self: &Arc<Self>, req: &Request) -> String {
        let header = |name: &str| {
            req.headers
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.as_str())
        };
        if !header("host").is_some_and(|h| self.hosts.iter().any(|a| a == h)) {
            return reply(403, "", "bad host");
        }
        let form = parse_form(if req.method == "POST" {
            &req.body
        } else {
            &req.query
        });
        // The secret buys a cookie once; links and forms never carry it.
        if req.method == "GET" && field(&form, "t") == self.token {
            return reply(
                303,
                &format!(
                    "Location: /\r\nSet-Cookie: {COOKIE}={}; HttpOnly; SameSite=Strict; Path=/\r\n",
                    self.token
                ),
                "",
            );
        }
        let cookie = format!("{COOKIE}={}", self.token);
        if !header("cookie").is_some_and(|c| c.split(';').any(|p| p.trim() == cookie)) {
            return reply(403, "", "open the URL that `fennec-os serve` printed");
        }
        let segments: Vec<&str> = req.path.trim_matches('/').split('/').collect();
        if !segments
            .iter()
            .all(|s| s.chars().all(|c| c.is_ascii_alphanumeric()))
        {
            return reply(404, "", "not found");
        }
        let page = match (req.method.as_str(), segments.as_slice()) {
            ("GET", [""]) => self.index(),
            ("GET", ["jobs"]) => Ok(self.jobs_frame()),
            ("GET", ["spec", id]) => self.spec_page(id),
            ("GET", ["run", id]) => self.run_page(id),
            ("POST", [kind @ ("spec" | "run" | "note"), rest @ ..]) => {
                return match self.decide(kind, rest, &form) {
                    Some(back) => reply(303, &format!("Location: {back}\r\n"), ""),
                    None => reply(404, "", "not found"),
                };
            }
            _ => return reply(404, "", "not found"),
        };
        match page {
            Ok(body) => reply(200, "", &body),
            Err(e) => reply(404, "", &e.to_string()),
        }
    }

    /// Starts the job a form asks for; returns where to go back to.
    fn decide(
        self: &Arc<Self>,
        kind: &str,
        rest: &[&str],
        form: &[(String, String)],
    ) -> Option<String> {
        let text = |k: &str| field(form, k).to_string();
        let attempts = field(form, "attempts")
            .parse()
            .unwrap_or(3usize)
            .clamp(1, 8);
        let id = rest.first().map(|s| s.to_string()).unwrap_or_default();
        let back = format!("/{kind}/{id}");
        match (kind, rest.get(1).copied().unwrap_or("")) {
            ("spec", "") if rest.is_empty() => {
                let (project, request) = (text("project"), text("request"));
                self.job(format!("compile \"{}\"", first_line(&request)), move |w| {
                    let model = OpenAi::variants_from_env(1)?.remove(0);
                    let s =
                        spec::compile(w, std::path::Path::new(&project), &request, &model, STEPS)?;
                    Ok(compiled(&s))
                });
                return Some("/".to_string());
            }
            ("spec", "ask") => {
                let question = text("question");
                self.job(format!("ask spec {id}"), move |w| {
                    let model = OpenAi::variants_from_env(1)?.remove(0);
                    spec::ask(w, &id, &question, &model)
                });
            }
            ("spec", "accept") => self.job(format!("solve spec {id}"), move |w| {
                let variants = OpenAi::variants_from_env(attempts)?;
                let models: Vec<&dyn Model> = variants.iter().map(|m| m as &dyn Model).collect();
                Ok(solved(&spec::accept(w, &id, &models, STEPS)?))
            }),
            ("spec", "reject") => {
                let (correction, again) = (text("correction"), !field(form, "again").is_empty());
                self.job(format!("reject spec {id}"), move |w| {
                    spec::reject(w, &id, Some(&correction))?;
                    if !again {
                        return Ok("rejected".to_string());
                    }
                    let old = spec::load(w, &id)?;
                    let project = old["project"].as_str().unwrap_or("");
                    let request = old["request"].as_str().unwrap_or("");
                    let model = OpenAi::variants_from_env(1)?.remove(0);
                    let s =
                        spec::compile(w, std::path::Path::new(project), request, &model, STEPS)?;
                    Ok(compiled(&s))
                });
                return Some("/".to_string());
            }
            ("run", "land") => self.job(format!("land {id}"), move |w| {
                solve::land(w, &id).map(|commit| format!("landed as {commit}"))
            }),
            ("run", "reject") => {
                let correction = text("correction");
                self.job(format!("reject run {id}"), move |w| {
                    let count = review::reject(w, &id, Some(&correction))?;
                    Ok(if count >= 2 {
                        format!("said {count} times; write it down once with `fennec-os promote`")
                    } else {
                        "rejected".to_string()
                    })
                });
            }
            ("note", "keep") => self.job(format!("keep {id}"), move |w| {
                sleep::keep(w, &id).map(|p| format!("added to {}; commit it", p.display()))
            }),
            ("note", "dismiss") => self.job(format!("dismiss {id}"), move |w| {
                sleep::dismiss(w, &id).map(|_| "dismissed".to_string())
            }),
            _ => return None,
        }
        Some(if kind == "note" {
            "/".to_string()
        } else {
            back
        })
    }

    fn job(
        self: &Arc<Self>,
        label: String,
        work: impl FnOnce(&Worlds) -> io::Result<String> + Send + 'static,
    ) {
        let index = {
            let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
            jobs.push(Job {
                label,
                outcome: None,
            });
            jobs.len() - 1
        };
        let server = Arc::clone(self);
        std::thread::spawn(move || {
            let outcome = work(&server.worlds).map_err(|e| e.to_string());
            server.jobs.lock().unwrap_or_else(|e| e.into_inner())[index].outcome = Some(outcome);
        });
    }

    /// Framed so it can refresh itself without wiping a form being typed.
    fn jobs_frame(&self) -> String {
        let jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        let mut body = String::new();
        for job in jobs.iter().rev().take(12) {
            let (class, text) = match &job.outcome {
                None => ("run", "running…"),
                Some(Ok(t)) => ("ok", t.as_str()),
                Some(Err(t)) => ("err", t.as_str()),
            };
            body.push_str(&format!(
                "<div class=\"{class}\"><b>{}</b> <pre>{}</pre></div>",
                esc(&job.label),
                esc(text)
            ));
        }
        if body.is_empty() {
            body.push_str("<p class=\"dim\">no job yet</p>");
        }
        let running = jobs.iter().any(|j| j.outcome.is_none());
        let refresh = if running {
            "<meta http-equiv=\"refresh\" content=\"3\">"
        } else {
            ""
        };
        format!(
            "<!doctype html><meta charset=\"utf-8\">{refresh}<style>{CSS}</style><body class=\"frame\">{body}"
        )
    }

    fn index(&self) -> io::Result<String> {
        let w = &self.worlds;
        let mut projects: Vec<String> = Vec::new();
        let mut body = String::from(
            "<h2>Jobs</h2><iframe src=\"/jobs\" title=\"jobs\"></iframe><h2>Specs to validate</h2>",
        );
        let specs = spec::specs(w)?;
        let runs = review::runs(w)?;
        for (_, v) in specs.iter().chain(runs.iter()) {
            let p = str_of(&v["project"]).to_string();
            if !p.is_empty() && !projects.contains(&p) {
                projects.push(p);
            }
        }
        let mut any = false;
        for (id, s) in specs
            .iter()
            .rev()
            .filter(|(_, s)| s["status"] == "proposed")
        {
            any = true;
            body.push_str(&format!(
                "<p><a href=\"/spec/{id}\">{id}</a> <span class=\"dim\">{}</span><br>{}</p>",
                esc(str_of(&s["project"])),
                esc(first_line(str_of(&s["request"])))
            ));
        }
        if !any {
            body.push_str("<p class=\"dim\">none</p>");
        }
        body.push_str("<h2>Runs waiting</h2>");
        let queue = review::queue(w)?;
        for (id, r) in queue.iter().rev() {
            body.push_str(&format!(
                "<p><a href=\"/run/{id}\">{id}</a> <b class=\"{0}\">{0}</b> <span class=\"dim\">{1}</span><br>{2}</p>",
                esc(str_of(&r["status"])),
                esc(str_of(&r["project"])),
                esc(first_line(str_of(&r["goal"])))
            ));
        }
        if queue.is_empty() {
            body.push_str("<p class=\"dim\">none</p>");
        }
        body.push_str("<h2>Know-how from sleep</h2>");
        let notes: Vec<Value> = sleep::candidates(w)?
            .into_iter()
            .filter(|c| c["status"] == "proposed")
            .collect();
        for c in &notes {
            let id = esc(str_of(&c["id"]));
            body.push_str(&format!(
                "<form method=\"post\" class=\"note\"><span class=\"dim\">{}</span> {}<br>\
                 <button formaction=\"/note/{id}/keep\">keep</button> \
                 <button formaction=\"/note/{id}/dismiss\">dismiss</button></form>",
                esc(str_of(&c["project"])),
                esc(str_of(&c["text"]))
            ));
        }
        if notes.is_empty() {
            body.push_str("<p class=\"dim\">none</p>");
        }
        let options: String = projects
            .iter()
            .map(|p| format!("<option value=\"{}\">", esc(p)))
            .collect();
        body.push_str(&format!(
            "<h2>New request</h2><form method=\"post\" action=\"/spec\">\
             <input name=\"project\" list=\"projects\" placeholder=\"/path/to/project\" required>\
             <datalist id=\"projects\">{options}</datalist>\
             <textarea name=\"request\" rows=\"4\" placeholder=\"what you want; tests come first\" required></textarea>\
             <button>compile into tests</button></form>"
        ));
        Ok(page("fennec-os", &body))
    }

    fn spec_page(&self, id: &str) -> io::Result<String> {
        let s = spec::load(&self.worlds, id)?;
        let mut body = format!(
            "<p><a href=\"/\">← back</a></p><h1>spec {id}</h1><p class=\"dim\">{} · {} {}</p>\
             <h2>Request</h2><pre>{}</pre><h2>Goal</h2><pre>{}</pre><h2>Verify</h2>",
            esc(str_of(&s["project"])),
            esc(str_of(&s["status"])),
            esc(str_of(&s["reason"])),
            esc(str_of(&s["request"])),
            esc(str_of(&s["goal"]))
        );
        for v in s["verify"].as_array().into_iter().flatten() {
            body.push_str(&format!("<pre>$ {}</pre>", esc(str_of(v))));
        }
        body.push_str(&checks_html(&s["checks"]));
        body.push_str(&format!("<h2>Tests</h2>{}", diff_html(str_of(&s["diff"]))));
        if s["thread"].is_array() {
            body.push_str("<h2>Questions</h2>");
        }
        for turn in s["thread"].as_array().into_iter().flatten() {
            body.push_str(&format!(
                "<p class=\"q\">{}</p><pre class=\"a\">{}</pre>",
                esc(str_of(&turn["question"])),
                esc(str_of(&turn["answer"]))
            ));
        }
        if s["status"] == "proposed" {
            body.push_str(&format!(
                "<iframe src=\"/jobs\" title=\"jobs\"></iframe>\
                 <form method=\"post\" action=\"/spec/{id}/ask\"><h2>Ask the compiler</h2>\
                 <textarea name=\"question\" rows=\"2\" required></textarea><button>ask</button></form>\
                 <form method=\"post\" action=\"/spec/{id}/accept\"><h2>Accept</h2>\
                 <label>attempts <input name=\"attempts\" type=\"number\" min=\"1\" max=\"8\" value=\"3\"></label> \
                 <button>accept and solve</button></form>\
                 <form method=\"post\" action=\"/spec/{id}/reject\"><h2>Reject</h2>\
                 <textarea name=\"correction\" rows=\"3\" placeholder=\"what is wrong; the next compile reads it\"></textarea>\
                 <button name=\"again\" value=\"1\">reject and recompile</button> <button>reject</button></form>"
            ));
        }
        Ok(page(&format!("spec {id}"), &body))
    }

    fn run_page(&self, id: &str) -> io::Result<String> {
        let r = review::load(&self.worlds, id)?;
        let mut body = format!(
            "<p><a href=\"/\">← back</a></p><h1>run {id}</h1><p class=\"dim\">{} · {}</p><pre>{}</pre>",
            esc(str_of(&r["project"])),
            esc(str_of(&r["status"])),
            esc(str_of(&r["goal"]))
        );
        for a in r["attempts"].as_array().into_iter().flatten() {
            let verdict = if a["passed"] == true { "PASS" } else { "FAIL" };
            body.push_str(&format!(
                "<h2>{} <b class=\"{verdict}\">{verdict}</b></h2><p class=\"dim\">{} {}</p><p>{}</p>",
                esc(str_of(&a["world"])),
                esc(str_of(&a["model"])),
                esc(str_of(&a["reason"])),
                esc(str_of(&a["summary"]))
            ));
            body.push_str(&checks_html(&a["checks"]));
            for v in a["violations"].as_array().into_iter().flatten() {
                body.push_str(&format!(
                    "<p class=\"FAIL\">rule {}: {}</p>",
                    esc(str_of(&v["rule"])),
                    esc(str_of(&v["violation"]))
                ));
            }
            if a["world"] == r["winner"] {
                body.push_str(&diff_html(str_of(&a["diff"])));
            }
        }
        let status = str_of(&r["status"]);
        if matches!(status, "ready" | "failed") {
            body.push_str("<iframe src=\"/jobs\" title=\"jobs\"></iframe>");
        }
        if status == "ready" {
            body.push_str(&format!(
                "<form method=\"post\" action=\"/run/{id}/land\"><button>land</button></form>"
            ));
        }
        if matches!(status, "ready" | "failed") {
            body.push_str(&format!(
                "<form method=\"post\" action=\"/run/{id}/reject\"><h2>Reject</h2>\
                 <textarea name=\"correction\" rows=\"3\" placeholder=\"what is wrong; the next runs read it\"></textarea>\
                 <button>reject</button></form>"
            ));
        }
        Ok(page(&format!("run {id}"), &body))
    }
}

fn compiled(s: &Value) -> String {
    let id = str_of(&s["world"]);
    if s["status"] == "proposed" {
        format!("spec {id} proposed")
    } else {
        format!("spec {id} invalid: {}", str_of(&s["reason"]))
    }
}

fn solved(record: &Value) -> String {
    match record["winner"].as_str() {
        Some(id) => format!("run {id} passed; review it to land"),
        None => "no attempt passed; the spec stays to retry".to_string(),
    }
}

fn checks_html(checks: &Value) -> String {
    checks
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| {
            let class = if c["exit"] == 0 { "PASS" } else { "FAIL" };
            format!(
                "<details><summary><b class=\"{class}\">exit {}</b> {}</summary><pre>{}</pre></details>",
                c["exit"],
                esc(str_of(&c["command"])),
                esc(str_of(&c["output"]))
            )
        })
        .collect()
}

fn diff_html(diff: &str) -> String {
    let lines: String = diff
        .lines()
        .map(|l| {
            let class = match l.as_bytes().first() {
                _ if l.starts_with("+++") || l.starts_with("---") => "file",
                Some(b'+') => "add",
                Some(b'-') => "del",
                Some(b'@') => "hunk",
                _ if l.starts_with("diff ") => "file",
                _ => "",
            };
            format!("<span class=\"{class}\">{}</span>\n", esc(l))
        })
        .collect();
    format!("<pre class=\"diff\">{lines}</pre>")
}

fn page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\">\
         <title>{}</title><style>{CSS}</style><main>{body}</main>",
        esc(title)
    )
}

fn reply(code: u16, headers: &str, body: &str) -> String {
    let reason = match code {
        200 => "OK",
        303 => "See Other",
        400 => "Bad Request",
        403 => "Forbidden",
        _ => "Not Found",
    };
    format!(
        "HTTP/1.1 {code} {reason}\r\n{headers}Content-Type: text/html; charset=utf-8\r\n\
         Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; frame-src 'self'; \
         form-action 'self'; frame-ancestors 'self'\r\n\
         Cache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn parse_form(text: &str) -> Vec<(String, String)> {
    text.split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (decode(k), decode(v)))
        .collect()
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(b) => {
                        out.push(b);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn field<'a>(form: &'a [(String, String)], name: &str) -> &'a str {
    form.iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
        .unwrap_or("")
}

fn str_of(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

const CSS: &str = "
:root{--bg:#fbfbf9;--fg:#1d1d1b;--dim:#6b6b66;--line:#e2e2dc;--add:#e6f4ea;--del:#fce8e6;--ok:#1e7a3c;--bad:#b3261e;--acc:#2f5bd3}
@media (prefers-color-scheme:dark){:root{--bg:#161615;--fg:#e8e8e3;--dim:#9a9a93;--line:#2c2c2a;--add:#16301f;--del:#3a1a17;--ok:#6fcf8f;--bad:#f2867c;--acc:#8fb0ff}}
body{background:var(--bg);color:var(--fg);font:15px/1.5 system-ui,sans-serif;margin:0}
main{max-width:980px;margin:0 auto;padding:16px}
a{color:var(--acc)}h1{font-size:1.4em}h2{font-size:1.05em;margin-top:1.6em;border-bottom:1px solid var(--line)}
pre{white-space:pre-wrap;word-break:break-word;font:13px/1.45 ui-monospace,monospace;margin:.3em 0}
.diff{border:1px solid var(--line);padding:8px;overflow-x:auto}.diff span{display:block}
.add{background:var(--add)}.del{background:var(--del)}.hunk{color:var(--acc)}.file{font-weight:bold}
.dim{color:var(--dim)}.PASS,.ok,.ready{color:var(--ok)}.FAIL,.err,.failed{color:var(--bad)}.run{color:var(--acc)}
textarea,input{width:100%;box-sizing:border-box;font:inherit;background:var(--bg);color:var(--fg);border:1px solid var(--line);padding:6px;margin:.3em 0}
input[type=number]{width:5em}button{font:inherit;padding:4px 12px;cursor:pointer}
iframe{width:100%;height:150px;border:1px solid var(--line)}
.frame{margin:6px;font-size:13px}.frame div{margin-bottom:4px}.frame pre{display:inline}
.q{font-weight:bold;margin-bottom:0}.note{margin:.6em 0}
";
