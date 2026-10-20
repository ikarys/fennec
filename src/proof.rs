//! Proof: before a run is called ready, the product is used the way its user
//! would use it, not through its tests. The model only proposes how to try
//! it; the harness runs the tries in the winner's world and keeps their real
//! output. A try exits 0 when the product behaves as the goal says, so the
//! verdict is still read from exit codes. What the tries cannot show is
//! written down as unverified, never left out.

use std::io;
use std::path::Path;
use std::process::Stdio;

use serde_json::{Value, json};

use crate::provider::Model;
use crate::review;
use crate::rules::{Kind, Rule};
use crate::solve;
use crate::team;
use crate::world::{World, Worlds};

/// More tries make the proof longer to read, not stronger.
const MAX_TRIES: usize = 5;
const MAX_SHOTS: usize = 3;

const BRIEF: &str = "A change was made to a project and its tests pass. Show that it works \
the way its user would see it: run the program or command, call the new code from a short \
script, start the server and query it. Do not run the test suite: it already passed, so \
never list it as unverified either. The commands run from the \
project root in a sandbox without network, so use only what the project or the machine \
already has: `npx` cannot fetch a package the project does not depend on. Anything you start \
must stop on its own (use `timeout`). Each command must print what a user would look at, and exit 0 only if the \
product behaves as the goal says. When the product is meant to fail (a bad argument, a \
usage line, no argument at all), write the try as `! <command>`: it passes only if the \
command fails. Never end a try with `; echo`, `|| true` or anything else that makes it \
exit 0 whatever the product did. \
If the project has a web interface the change shows in, also say how to start it and which \
pages to capture: the harness starts it, opens each page in a real browser and takes a \
screenshot, so never imitate a browser or a DOM in a try: when the change only shows in \
the web interface, give serve and shot lines and no try. When the change shows only after \
the user does something on the page, follow its shot line with act lines doing it, in \
order: the harness does them with a real mouse and keyboard and captures the page before \
and after. When a \"Done when\" is given, it is what the person validated: show every \
action and result it names (each click it describes is an act, a click that undoes \
another is an act too), and give an unverified line for each one you cannot show. \
Keep each command short. Reply in exactly this form, nothing else:\n\
try: <one shell command>   (one line per command, at most 5)\n\
serve: <shell command that starts the web interface and keeps running>   (omit if none)\n\
shot: <page URL on 127.0.0.1, port included>   (one line per page, at most 3, only with serve)\n\
act: one action per line, either `click <text the user sees on what they click>`, \
`type <field's placeholder or label> => <text>` or `key <Enter, Tab, Escape or Backspace>`   \
(after its shot line, at most 10 per page)\n\
unverified: <what these cannot show, for the person: in the language of the Done when \
(or the goal), product terms, no file or tag names>   (one line per item, omit if none)";

/// Drives the browser inside the sandbox; stdlib Python, see the file.
const DRIVER: &str = include_str!("proof.py");
const MAX_STEPS: usize = 10;

/// Starts the product, waits for its port, then uses the page with the
/// browser. The driver writes the PNGs to stdout and what it saw to stderr.
/// Arguments: serve, url, port, driver, steps as JSON.
const SHOT_SCRIPT: &str = r#"
serve="$1"; url="$2"; port="$3"
( exec sh -c "$serve" ) >/tmp/serve.log 2>&1 &
up() { (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; }
for _ in $(seq 60); do up && break; sleep 0.5; done
if ! up; then echo "nothing listens on port $port after 30 s; server output:" >&2; tail -20 /tmp/serve.log >&2; exit 2; fi
exec timeout 90 python3 -I -c "$4" "$url" "$5"
"#;

#[derive(Debug, Default, PartialEq)]
struct Plan {
    tries: Vec<String>,
    serve: Option<String>,
    /// Each page with the steps done on it before the capture.
    shots: Vec<(String, Vec<String>)>,
    unverified: Vec<String>,
}

/// Gathers the proof for the run `id` (its winner's world) and saves it in
/// the run record. Screenshots are kept under `<root>/proofs/`. Returns
/// `{"tries": [...], "shots": [...], "unverified": [...], "passed": bool}`.
/// `done_when` is what the human validated: the proof must show all of it.
pub fn gather(
    worlds: &Worlds,
    id: &str,
    done_when: Option<&str>,
    model: &dyn Model,
) -> io::Result<Value> {
    let mut record = review::load(worlds, id)?;
    let world = worlds.open(id)?;
    let diff = record["attempts"]
        .as_array()
        .and_then(|a| a.iter().find(|t| t["world"] == id))
        .and_then(|t| t["diff"].as_str())
        .unwrap_or("");
    let done = done_when.map_or_else(String::new, |d| format!("Done when:\n{d}\n\n"));
    // The reminder comes last: the brief alone, in English, was not enough to
    // keep unverified lines out of English for a French Done when.
    let request = format!(
        "{done}Goal:\n{}\n\nChange:\n{diff}\n\nREADME:\n{}\n\n\
         The person reads only the language of «{}»: write each unverified line in it, \
         not in the language of this message.",
        record["goal"].as_str().unwrap_or(""),
        readme(worlds, &world)?,
        // Their own words name the language better than any example would:
        // a French example leaked into English proofs.
        done_when
            .unwrap_or(record["goal"].as_str().unwrap_or(""))
            .chars()
            .take(80)
            .collect::<String>()
    );
    let project = Path::new(record["project"].as_str().unwrap_or(""));
    let (roles, team) = team::brief(project, "prove")?;
    let messages = [
        json!({"role": "system", "content": format!("{BRIEF}{roles}")}),
        json!({"role": "user", "content": request}),
    ];
    // A model can spend the whole reply reasoning (seen: 8192 tokens of
    // thinking, no answer); `ask` then plans again without.
    let (content, exchange) = crate::provider::ask(model, &messages)?;
    let mut plan = parse(&content);

    let command = |c: &String| Rule {
        id: "proof.try".to_string(),
        kind: Kind::StandingCommand(c.clone()),
        block: true,
    };
    let rules: Vec<Rule> = plan.tries.iter().map(command).collect();
    // A try may write files: the world is rolled back below rather than the
    // proof failed, so the second half of verify's answer is not used.
    let (checks, _) = solve::verify(worlds, &world, &rules)?;
    worlds.rollback(&world)?;
    // The roles' own checks: chosen by the human, not the model, so they
    // do not count as trying the product, but any failure is a veto. A tool
    // the machine does not have (exit 127) is not verified, not a failure.
    let vetoes = team::checks(project)?;
    let veto_rules: Vec<Rule> = vetoes.iter().map(|(_, c)| command(c)).collect();
    let (mut role_checks, _) = solve::verify(worlds, &world, &veto_rules)?;
    worlds.rollback(&world)?;
    for (check, (role, _)) in role_checks.iter_mut().zip(&vetoes) {
        check["role"] = json!(role);
        check["missing"] = json!(check["exit"] == 127);
    }
    let mut shots = Vec::new();
    if let Some(serve) = &plan.serve {
        for (n, (url, steps)) in plan.shots.iter().enumerate() {
            shots.push(shoot(
                worlds,
                &world,
                serve,
                url,
                steps,
                &format!("{id}-{n}"),
            )?);
            worlds.rollback(&world)?;
        }
    }
    if checks.is_empty() && shots.is_empty() {
        plan.unverified.push(if content.trim().is_empty() {
            // Reasoning models can spend the whole budget thinking.
            "no try proposed: the model's reply was empty or cut off".to_string()
        } else {
            "no try proposed".to_string()
        });
    }
    // A page this machine has no browser to look at is unverified, not
    // failed; it proves nothing either, so something else must.
    let seen: Vec<&Value> = shots.iter().filter(|s| s["missing"] != true).collect();
    let passed = !(checks.is_empty() && seen.is_empty())
        && checks.iter().all(|c| c["exit"] == 0)
        && seen.iter().all(|s| s["ok"] == true)
        && role_checks
            .iter()
            .all(|c| c["exit"] == 0 || c["missing"] == true);
    let proof = json!({"tries": checks, "shots": shots, "unverified": plan.unverified,
                       "checks": role_checks, "passed": passed, "team": team,
                       "serve": plan.serve, "exchange": exchange});
    record["proof"] = proof.clone();
    review::save(worlds, id, &record)?;
    Ok(proof)
}

/// The thread that holds the page started by `page`: dropping this sender
/// ends it, and the sandbox with it (`--die-with-parent` follows the thread
/// that spawned bwrap; killing bwrap alone leaves the sandbox running).
/// ponytail: one page at a time for the whole process (the proofs' servers
/// tend to share a port); a map per run if several must stay open.
static PAGE: std::sync::Mutex<Option<std::sync::mpsc::Sender<()>>> = std::sync::Mutex::new(None);

/// The run's product started the way its proof started it, for the human to
/// use in their own browser: the same sandbox, but on the host's network so
/// the page can be reached. Returns the page once it answers. The server
/// stops after 30 minutes, when another page starts, or with the caller.
pub fn page(worlds: &Worlds, id: &str) -> io::Result<String> {
    let record = review::load(worlds, id)?;
    let serve = record["proof"]["serve"]
        .as_str()
        .ok_or_else(|| io::Error::other("the proof started no web interface"))?;
    let url = record["proof"]["shots"][0]["url"]
        .as_str()
        .ok_or_else(|| io::Error::other("the proof captured no page"))?;
    let port = local_port(url)
        .ok_or_else(|| io::Error::other(format!("{url} is not a page on this machine")))?;
    let world = worlds.open(id)?;
    let mut page = PAGE.lock().unwrap_or_else(|e| e.into_inner());
    if page.take().is_some() {
        // The old thread sees the drop within its next half second.
        std::thread::sleep(std::time::Duration::from_millis(700));
    }
    let up = |port| std::net::TcpStream::connect(("127.0.0.1", port)).is_ok();
    // The proof ran in its own network, where every port was free; the
    // human's machine has its own servers (seen: the model server on 8000).
    let (mut serve, mut url) = (untimed(serve), url.to_string());
    if up(port) {
        let free = std::net::TcpListener::bind(("127.0.0.1", 0))?
            .local_addr()?
            .port();
        serve = swap_port(&serve, port, free)
            .ok_or_else(|| io::Error::other(format!("port {port} is already taken")))?;
        url = url.replacen(&format!(":{port}"), &format!(":{free}"), 1);
    }
    let port = local_port(&url).unwrap_or(port);
    let argv = ["timeout", "1800", "sh", "-c", &serve].map(String::from);
    let mut command = worlds.sandbox_on_host_network(&world, &argv)?;
    let dir = worlds.root.join("proofs");
    std::fs::create_dir_all(&dir)?;
    let log = dir.join(format!("{id}-page.log"));
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&log)?);
    let (stop, stopped) = std::sync::mpsc::channel::<()>();
    let (started, start) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => return started.send(Err(e)),
        };
        let _ = started.send(Ok(()));
        while stopped.recv_timeout(std::time::Duration::from_millis(500))
            == Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            && child.try_wait().is_ok_and(|s| s.is_none())
        {}
        let _ = child.kill();
        let _ = child.wait();
        Ok(())
    });
    start.recv().map_err(io::Error::other)??;
    for _ in 0..60 {
        if up(port) {
            *page = Some(stop);
            return Ok(url);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    drop(stop);
    let log = std::fs::read_to_string(&log).unwrap_or_default();
    let tail: Vec<&str> = log.lines().collect();
    Err(io::Error::other(format!(
        "nothing listens on port {port} after 30 s: {}",
        tail[tail.len().saturating_sub(5)..].join("\n")
    )))
}

/// `serve` without the `timeout` the proof's brief asks for: the human's
/// page would close under them; `page` sets its own.
/// ponytail: a leading `timeout` only.
fn untimed(serve: &str) -> String {
    let mut words = serve.split_whitespace().peekable();
    if words.peek() != Some(&"timeout") {
        return serve.to_string();
    }
    words.next();
    while let Some(w) = words.next_if(|w| w.starts_with('-')) {
        if matches!(w, "-k" | "-s") {
            words.next();
        }
    }
    words.next();
    words.collect::<Vec<_>>().join(" ")
}

/// `serve` with the port number `old` said as `new`, when it says it.
/// ponytail: the number in the command only; a port written in the
/// project's code stays taken.
fn swap_port(serve: &str, old: u16, new: u16) -> Option<String> {
    let old = old.to_string();
    let mut out = String::new();
    let mut found = false;
    let mut rest = serve;
    while let Some(at) = rest.find(&old) {
        let before = rest[..at].chars().next_back();
        let after = rest[at + old.len()..].chars().next();
        out.push_str(&rest[..at]);
        if before.is_some_and(|c| c.is_ascii_digit()) || after.is_some_and(|c| c.is_ascii_digit()) {
            out.push_str(&old);
        } else {
            out.push_str(&new.to_string());
            found = true;
        }
        rest = &rest[at + old.len()..];
    }
    out.push_str(rest);
    found.then_some(out)
}

/// How the project says it is run: its README, then AGENTS.md, where a
/// project without a README says it (QA served a file listing instead).
/// Read inside the sandbox, like everything taken from a world.
fn readme(worlds: &Worlds, world: &World) -> io::Result<String> {
    let argv = [
        "sh",
        "-c",
        "cat README* readme* AGENTS.md 2>/dev/null | head -c 6000",
    ]
    .map(String::from);
    let out = worlds
        .sandbox_command(world, &argv)?
        .stdin(Stdio::null())
        .output()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if text.is_empty() {
        "(none)".to_string()
    } else {
        text
    })
}

/// One page used and captured: `<name>.png`, and `<name>-before.png` when
/// steps were done on it. The PNGs come back through the sandbox's stdout:
/// the harness never reads a file in the world, so a symlink there leads
/// nowhere. A step that finds nothing to act on fails the shot.
fn shoot(
    worlds: &Worlds,
    world: &World,
    serve: &str,
    url: &str,
    steps: &[String],
    name: &str,
) -> io::Result<Value> {
    let mut shot = json!({"url": url, "steps": steps, "ok": false});
    let Some(port) = local_port(url) else {
        shot["reason"] = json!("not a page on 127.0.0.1 or localhost");
        return Ok(shot);
    };
    let argv: Vec<String> = [
        "timeout",
        "-k",
        "5",
        "150",
        "bash",
        "-c",
        SHOT_SCRIPT,
        "shot",
    ]
    .iter()
    .map(|s| s.to_string())
    .chain([
        serve.to_string(),
        url.to_string(),
        port.to_string(),
        DRIVER.to_string(),
        json!(steps).to_string(),
    ])
    .collect();
    let out = worlds
        .sandbox_command(world, &argv)?
        .stdin(Stdio::null())
        .output()?;
    let log = String::from_utf8_lossy(&out.stderr);
    let seen: Value = log
        .lines()
        .find_map(|l| l.strip_prefix("proof="))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or(Value::Null);
    let sizes: Vec<usize> = seen["sizes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| n.as_u64().map(|n| n as usize))
        .collect();
    if seen["missing"] == true {
        shot["missing"] = json!(true);
        shot["reason"] = json!("no browser");
        return Ok(shot);
    }
    if !out.status.success() || sizes.is_empty() || sizes.iter().sum::<usize>() != out.stdout.len()
    {
        shot["reason"] = json!(seen["error"].as_str().map_or_else(
            || {
                let tail: Vec<&str> = log.lines().collect();
                tail[tail.len().saturating_sub(20)..].join("\n")
            },
            str::to_string
        ));
        return Ok(shot);
    }
    let dir = worlds.root.join("proofs");
    std::fs::create_dir_all(&dir)?;
    let (before, after) = out
        .stdout
        .split_at(out.stdout.len() - sizes[sizes.len() - 1]);
    if !before.is_empty() {
        std::fs::write(dir.join(format!("{name}-before.png")), before)?;
        shot["before"] = json!(format!("{name}-before.png"));
        // Steps that change nothing on screen show nothing of the work
        // (UX test 2026-10-18: a task added then deleted, "ready" in green).
        shot["unchanged"] = json!(before == after);
    }
    std::fs::write(dir.join(format!("{name}.png")), after)?;
    shot["file"] = json!(format!("{name}.png"));
    // An empty page is the classic broken UI that still "passes".
    // ponytail: visible text only; a canvas-only page would need pixel checks.
    if seen["text"] == 0 {
        shot["reason"] = json!("the page renders no text");
    } else {
        shot["ok"] = json!(true);
    }
    Ok(shot)
}

/// The port of a page on this machine; the sandbox reaches nothing else.
fn local_port(url: &str) -> Option<u16> {
    let rest = url.strip_prefix("http://")?;
    let host = rest.split('/').next()?;
    let (name, port) = host.rsplit_once(':').unwrap_or((host, "80"));
    matches!(name, "127.0.0.1" | "localhost")
        .then(|| port.parse().ok())
        .flatten()
}

fn parse(reply: &str) -> Plan {
    let mut plan = Plan::default();
    for line in reply.lines() {
        let line = line.trim().trim_start_matches("- ");
        let clean = |s: &str| s.trim().trim_matches('`').trim().to_string();
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = clean(value);
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "try" if plan.tries.len() < MAX_TRIES => plan.tries.push(value),
            "serve" => plan.serve = Some(value),
            "shot" if plan.shots.len() < MAX_SHOTS => plan.shots.push((value, Vec::new())),
            "act" => {
                if let Some((_, steps)) = plan.shots.last_mut() {
                    // Models quote what they aim at (`click `Save``), and
                    // some chain actions on one line: `click A | click B`.
                    for step in value.split(" | ") {
                        if steps.len() < MAX_STEPS {
                            steps.push(step.replace('`', "").trim().to_string());
                        }
                    }
                }
            }
            "unverified" if !value.eq_ignore_ascii_case("none") => plan.unverified.push(value),
            _ => {}
        }
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::{local_port, parse, swap_port, untimed};

    #[test]
    fn the_page_drops_the_proofs_timeout() {
        assert_eq!(
            untimed("timeout 30 python3 -m http.server 8000"),
            "python3 -m http.server 8000"
        );
        assert_eq!(untimed("timeout -k 5 30s npm start"), "npm start");
        assert_eq!(untimed("npm start"), "npm start");
    }

    #[test]
    fn a_taken_port_is_swapped_where_the_command_says_it() {
        assert_eq!(
            swap_port("python3 -m http.server 8000 # 18000", 8000, 4242).as_deref(),
            Some("python3 -m http.server 4242 # 18000")
        );
        assert_eq!(
            swap_port("PORT=8000 node s.js --port=8000", 8000, 9).as_deref(),
            Some("PORT=9 node s.js --port=9")
        );
        assert_eq!(swap_port("node server.js", 8000, 9), None);
    }

    #[test]
    fn reply_parsing() {
        let plan = parse(
            "Sure.\ntry: `python3 -c 'print(1)'`\n- try: ./run.sh\nserve: `python3 -m http.server 8000`\nshot: http://127.0.0.1:8000/\nunverified: the GUI\nunverified: none\n",
        );
        assert_eq!(plan.tries, ["python3 -c 'print(1)'", "./run.sh"]);
        assert_eq!(plan.serve.as_deref(), Some("python3 -m http.server 8000"));
        assert_eq!(plan.shots, [("http://127.0.0.1:8000/".to_string(), vec![])]);
        assert_eq!(plan.unverified, ["the GUI"]);
        let many = "try: a\n".repeat(9);
        assert_eq!(parse(&many).tries.len(), 5);
    }

    #[test]
    fn steps_belong_to_the_page_before_them() {
        let plan = parse(
            "act: click orphan\nshot: http://127.0.0.1:8000/\nact: click `pain`\nact: type Nouvel article => lait | key Enter\nshot: http://127.0.0.1:8000/b\n",
        );
        assert_eq!(
            plan.shots,
            [
                (
                    "http://127.0.0.1:8000/".to_string(),
                    vec![
                        "click pain".to_string(),
                        "type Nouvel article => lait".to_string(),
                        "key Enter".to_string()
                    ]
                ),
                ("http://127.0.0.1:8000/b".to_string(), vec![])
            ]
        );
    }

    #[test]
    fn only_local_pages_are_shot() {
        assert_eq!(local_port("http://127.0.0.1:8000/x"), Some(8000));
        assert_eq!(local_port("http://localhost/"), Some(80));
        assert_eq!(local_port("http://example.com:8000/"), None);
        assert_eq!(local_port("https://127.0.0.1:8000/"), None);
    }
}
