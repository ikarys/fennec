mod common;

use std::collections::VecDeque;
use std::io;
use std::sync::Mutex;
use std::time::Duration;

use common::Fixture;
use fennec_os::chat;
use fennec_os::provider::Model;
use fennec_os::world::Worlds;
use serde_json::{Value, json};

/// Replies in order, raw.
struct Replies(Mutex<VecDeque<Value>>);

impl Model for Replies {
    fn chat(&self, _messages: &[Value], _tools: &Value) -> io::Result<Value> {
        self.0
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| io::Error::other("script exhausted"))
    }
    fn label(&self) -> String {
        "replies".into()
    }
    fn variant<'s>(&'s self, _: &str) -> Box<dyn Model + 's> {
        Box::new(Replies(Mutex::new(VecDeque::new())))
    }
}

/// The messages the page shows: what the harness added (the mode, the
/// reminders, the nudges) aside.
fn said(c: &Value) -> Vec<Value> {
    c["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["harness"] != true)
        .cloned()
        .collect()
}

/// The harness's nudges back to the work: neither the mode nor a reminder.
fn sent_back(c: &Value) -> Vec<Value> {
    c["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| {
            let t = m["content"].as_str().unwrap_or("");
            m["harness"] == true && m["mode"] != true && !t.starts_with("<system-reminder>\nStep")
        })
        .cloned()
        .collect()
}

fn call(id: &str, name: &str, args: Value) -> Value {
    json!({"content": null, "tool_calls": [{"id": id, "type": "function",
           "function": {"name": name, "arguments": args.to_string()}}]})
}

#[test]
fn a_chat_reads_and_writes_its_folder_and_ends_on_a_reply() {
    let f = Fixture::new("chat");
    // A plain folder, no git, inside the home.
    let folder = f.worlds.home.join("notes");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.txt"), "alpha\n").unwrap();
    let c = chat::start(&f.worlds, &folder).unwrap();
    let id = c["id"].as_str().unwrap();
    chat::write(&f.worlds, id, true).unwrap();
    chat::say(&f.worlds, id, "Copie a.txt dans b.txt").unwrap();
    assert!(
        chat::say(&f.worlds, id, "encore").is_err(),
        "one turn at a time"
    );
    let model = Replies(Mutex::new(VecDeque::from([
        call(
            "c0",
            "shell",
            json!({"command": "cat a.txt; cat ../secret"}),
        ),
        call(
            "c1",
            "write_file",
            json!({"path": "b.txt", "content": "alpha\n"}),
        ),
        json!({"content": "C'est fait."}),
    ])));
    chat::answer(&f.worlds, id, &model, 10).unwrap();
    assert_eq!(
        std::fs::read_to_string(folder.join("b.txt")).unwrap(),
        "alpha\n"
    );
    let c = chat::load(&f.worlds, id).unwrap();
    assert_eq!(c["status"], "idle");
    assert!(c["error"].is_null(), "{}", c["error"]);
    let messages = said(&c);
    let read = messages[2]["content"].as_str().unwrap();
    assert!(read.contains("alpha"), "{read}");
    assert!(
        !read.contains("s3cret"),
        "the rest of the home stays hidden"
    );
    assert_eq!(messages.last().unwrap()["content"], "C'est fait.");
    assert_eq!(c["turns"].as_array().unwrap().len(), 3);
    assert_eq!(
        chat::list(&f.worlds).unwrap()[0]["title"],
        "Copie a.txt dans b.txt"
    );
    // The home itself is never a chat's folder.
    assert!(chat::start(&f.worlds, &f.worlds.home).is_err());
}

#[test]
fn a_chat_starts_read_only_until_changes_are_allowed() {
    let f = Fixture::new("chat-read");
    let folder = f.worlds.home.join("notes");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.txt"), "alpha\n").unwrap();
    let c = chat::start(&f.worlds, &folder).unwrap();
    let id = c["id"].as_str().unwrap();
    chat::say(&f.worlds, id, "Copie a.txt dans b.txt").unwrap();
    let model = Replies(Mutex::new(VecDeque::from([
        call(
            "c0",
            "shell",
            json!({"command": "cat a.txt && cp a.txt b.txt"}),
        ),
        json!({"content": "Voici le plan."}),
    ])));
    chat::answer(&f.worlds, id, &model, 10).unwrap();
    assert!(!folder.join("b.txt").exists(), "read-only by default");
    let c = chat::load(&f.worlds, id).unwrap();
    let shown = said(&c);
    let ran = shown[2]["content"].as_str().unwrap();
    assert!(ran.contains("alpha") && ran.contains("Read-only"), "{ran}");
    // Allowed: the same command now writes.
    chat::write(&f.worlds, id, true).unwrap();
    chat::say(&f.worlds, id, "vas-y").unwrap();
    let model = Replies(Mutex::new(VecDeque::from([
        call("c1", "shell", json!({"command": "cp a.txt b.txt"})),
        json!({"content": "C'est fait."}),
    ])));
    chat::answer(&f.worlds, id, &model, 10).unwrap();
    assert!(folder.join("b.txt").exists());
}

/// A second handle on the same store, for the thread the turn runs in.
fn twin(f: &Fixture) -> Worlds {
    Worlds {
        root: f.worlds.root.clone(),
        home: f.worlds.home.clone(),
        ro_paths: Vec::new(),
        memory_mb: Some(8192),
        apart: false,
        base: None,
    }
}

/// Polls the chat file until the turn shows its question; the turn ends
/// without asking, so that is a failure.
fn until_waiting(f: &Fixture, id: &str) -> Value {
    let w = twin(f);
    loop {
        let c = chat::load(&w, id).unwrap();
        if c["status"] == "idle" {
            panic!("ended without asking: {}", c["error"]);
        }
        if c["status"] == "waiting" {
            return c;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The fixture's home must not sit under a path the sandbox hides (its
/// /tmp and friends are private tmpfs): the question is about host paths.
/// `/var/tmp` when it can be written; inside a fennec world it is read-only,
/// and cargo's own scratch folder under the target dir stands in.
fn var_tmp() -> std::path::PathBuf {
    let p = std::path::PathBuf::from("/var/tmp");
    let probe = p.join(format!(".fennec-os-probe-{}", std::process::id()));
    if std::fs::write(&probe, "").is_ok() {
        let _ = std::fs::remove_file(&probe);
        return p;
    }
    std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
}

#[test]
fn an_out_of_folder_path_is_asked_and_the_folder_grant_spares_the_next_call() {
    let f = Fixture::new_in("chat-grant", &var_tmp());
    let folder = f.worlds.home.join("notes");
    std::fs::create_dir_all(&folder).unwrap();
    let other = f.worlds.home.join("other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("o.txt"), "omega\n").unwrap();
    let c = chat::start(&f.worlds, &folder).unwrap();
    let id = c["id"].as_str().unwrap().to_string();
    chat::write(&f.worlds, &id, true).unwrap();
    chat::say(&f.worlds, &id, "lis ../other/o.txt").unwrap();
    let model = Replies(Mutex::new(VecDeque::from([
        call("c0", "shell", json!({"command": "cat ../other/o.txt"})),
        call(
            "c1",
            "write_file",
            json!({"path": "../other/n.txt", "content": "x\n"}),
        ),
        json!({"content": "C'est fait."}),
    ])));
    let w = twin(&f);
    let tid = id.clone();
    let t = std::thread::spawn(move || chat::answer(&w, &tid, &model, 10));
    let pending = until_waiting(&f, &id);
    // The question names the path as written and the folder that opens.
    assert_eq!(
        pending["pending"]["path"],
        other.join("o.txt").display().to_string()
    );
    assert_eq!(pending["pending"]["directory"], other.display().to_string());
    // One exchange at a time: the question is where the person's words go.
    assert!(chat::say(&f.worlds, &id, "encore").is_err());
    chat::decide(&f.worlds, &id, true).unwrap();
    t.join().unwrap().unwrap();
    let c = chat::load(&f.worlds, &id).unwrap();
    assert_eq!(c["status"], "idle");
    assert!(c["error"].is_null(), "{}", c["error"]);
    // The folder is the grant: the second call ran read-write, no second ask.
    assert_eq!(c["grants"], json!([other.display().to_string()]));
    assert_eq!(std::fs::read_to_string(other.join("n.txt")).unwrap(), "x\n");
    let read = c["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "tool")
        .unwrap()["content"]
        .as_str()
        .unwrap();
    assert!(read.contains("omega"), "{read}");
    // Nothing waits once the turn is idle.
    assert!(chat::decide(&f.worlds, &id, true).is_err());
}

#[test]
fn a_refused_path_is_not_touched_and_asks_again() {
    let f = Fixture::new_in("chat-refuse", &var_tmp());
    let folder = f.worlds.home.join("notes");
    std::fs::create_dir_all(&folder).unwrap();
    let other = f.worlds.home.join("other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("o.txt"), "omega\n").unwrap();
    let c = chat::start(&f.worlds, &folder).unwrap();
    let id = c["id"].as_str().unwrap().to_string();
    chat::write(&f.worlds, &id, true).unwrap();
    chat::say(&f.worlds, &id, "ecris ../other/n.txt").unwrap();
    let model = Replies(Mutex::new(VecDeque::from([
        call(
            "c0",
            "write_file",
            json!({"path": "../other/n.txt", "content": "x\n"}),
        ),
        call("c1", "shell", json!({"command": "cat ../other/o.txt"})),
        json!({"content": "ok."}),
    ])));
    let w = twin(&f);
    let tid = id.clone();
    let t = std::thread::spawn(move || chat::answer(&w, &tid, &model, 10));
    // First question: refused. The call must not have run.
    until_waiting(&f, &id);
    chat::decide(&f.worlds, &id, false).unwrap();
    assert!(
        !other.join("n.txt").exists(),
        "a refusal does not run the call"
    );
    let c = chat::load(&f.worlds, &id).unwrap();
    assert!(c["grants"].as_array().unwrap().is_empty());
    // The same folder asks again: a refusal covers its call only.
    let pending = until_waiting(&f, &id);
    assert_eq!(pending["pending"]["directory"], other.display().to_string());
    chat::decide(&f.worlds, &id, true).unwrap();
    t.join().unwrap().unwrap();
    let c = chat::load(&f.worlds, &id).unwrap();
    assert_eq!(c["status"], "idle");
    assert!(c["error"].is_null(), "{}", c["error"]);
    assert_eq!(c["grants"], json!([other.display().to_string()]));
    // The refusal is the refused call's tool result.
    let shown = said(&c);
    let refused = &shown[2];
    assert_eq!(refused["role"], "tool");
    assert!(refused["content"].as_str().unwrap().contains("refused"));
}

#[test]
fn a_stop_while_waiting_ends_the_turn_without_running_the_call() {
    let f = Fixture::new_in("chat-stop", &var_tmp());
    let folder = f.worlds.home.join("notes");
    std::fs::create_dir_all(&folder).unwrap();
    let other = f.worlds.home.join("other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("o.txt"), "omega\n").unwrap();
    let c = chat::start(&f.worlds, &folder).unwrap();
    let id = c["id"].as_str().unwrap().to_string();
    chat::write(&f.worlds, &id, true).unwrap();
    chat::say(&f.worlds, &id, "ecris ../other/n.txt").unwrap();
    let model = Replies(Mutex::new(VecDeque::from([
        call(
            "c0",
            "write_file",
            json!({"path": "../other/n.txt", "content": "x\n"}),
        ),
        json!({"content": "ok."}),
    ])));
    let w = twin(&f);
    let tid = id.clone();
    let t = std::thread::spawn(move || chat::answer(&w, &tid, &model, 10));
    // The question is on the page; the person stops the turn instead.
    until_waiting(&f, &id);
    chat::stop(&f.worlds, &id).unwrap();
    t.join().unwrap().unwrap();
    let c = chat::load(&f.worlds, &id).unwrap();
    assert_eq!(c["status"], "idle");
    assert!(c["error"].is_null(), "{}", c["error"]);
    assert!(c["pending"].is_null());
    assert!(c["grants"].as_array().unwrap().is_empty());
    assert!(!other.join("n.txt").exists(), "the stopped call never ran");
}

#[test]
fn the_home_or_the_root_is_refused_without_asking() {
    let f = Fixture::new_in("chat-home", &var_tmp());
    let folder = f.worlds.home.join("notes");
    std::fs::create_dir_all(&folder).unwrap();
    let c = chat::start(&f.worlds, &folder).unwrap();
    let id = c["id"].as_str().unwrap().to_string();
    chat::write(&f.worlds, &id, true).unwrap();
    chat::say(&f.worlds, &id, "ecris ~/n.txt").unwrap();
    let model = Replies(Mutex::new(VecDeque::from([
        call(
            "c0",
            "write_file",
            json!({"path": "~/n.txt", "content": "x\n"}),
        ),
        call("c1", "shell", json!({"command": "ls /"})),
        json!({"content": "ok."}),
    ])));
    // No question to answer: the turn runs to its end on its own.
    chat::answer(&twin(&f), &id, &model, 10).unwrap();
    let c = chat::load(&f.worlds, &id).unwrap();
    assert!(c["error"].is_null(), "{}", c["error"]);
    assert!(c["grants"].as_array().unwrap().is_empty());
    assert!(!f.worlds.home.join("n.txt").exists());
    let refused: Vec<&str> = c["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "tool")
        .filter_map(|m| m["content"].as_str())
        .collect();
    assert_eq!(refused.len(), 2);
    assert!(
        refused.iter().all(|r| r.contains("refused without asking")),
        "{refused:?}"
    );
}

/// Replies in order, taking `dir` back from the chat before the second one,
/// as the person would while the turn runs.
struct UngrantMidTurn {
    replies: Replies,
    worlds: Worlds,
    id: String,
    dir: String,
    calls: Mutex<usize>,
}

impl Model for UngrantMidTurn {
    fn chat(&self, messages: &[Value], tools: &Value) -> io::Result<Value> {
        let mut n = self.calls.lock().unwrap();
        *n += 1;
        if *n == 2 {
            chat::ungrant(&self.worlds, &self.id, &self.dir)?;
        }
        self.replies.chat(messages, tools)
    }
    fn label(&self) -> String {
        "ungrant".into()
    }
    fn variant<'s>(&'s self, _: &str) -> Box<dyn Model + 's> {
        Box::new(UngrantMidTurn {
            replies: Replies(Mutex::new(VecDeque::new())),
            worlds: self.worlds.clone(),
            id: self.id.clone(),
            dir: self.dir.clone(),
            calls: Mutex::new(0),
        })
    }
}

#[test]
fn a_folder_taken_back_closes_within_the_turn() {
    let f = Fixture::new_in("chat-ungrant", &var_tmp());
    let folder = f.worlds.home.join("notes");
    std::fs::create_dir_all(&folder).unwrap();
    let other = f.worlds.home.join("other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("o.txt"), "omega\n").unwrap();
    let c = chat::start(&f.worlds, &folder).unwrap();
    let id = c["id"].as_str().unwrap().to_string();
    chat::write(&f.worlds, &id, true).unwrap();
    chat::say(&f.worlds, &id, "lis puis ecris").unwrap();
    let model = UngrantMidTurn {
        replies: Replies(Mutex::new(VecDeque::from([
            call("c0", "shell", json!({"command": "cat ../other/o.txt"})),
            call(
                "c1",
                "write_file",
                json!({"path": "../other/n.txt", "content": "x\n"}),
            ),
            json!({"content": "ok."}),
        ]))),
        worlds: twin(&f),
        id: id.clone(),
        dir: other.display().to_string(),
        calls: Mutex::new(0),
    };
    let w = twin(&f);
    let tid = id.clone();
    let t = std::thread::spawn(move || chat::answer(&w, &tid, &model, 10));
    until_waiting(&f, &id);
    chat::decide(&f.worlds, &id, true).unwrap();
    // Taken back before the write: the turn asks again instead of writing.
    until_waiting(&f, &id);
    chat::decide(&f.worlds, &id, false).unwrap();
    t.join().unwrap().unwrap();
    assert!(
        !other.join("n.txt").exists(),
        "a folder taken back stays closed"
    );
}

#[test]
fn a_turn_cut_by_a_restart_lets_the_person_speak_again() {
    let f = Fixture::new("chat-recover");
    let folder = f.worlds.home.join("notes");
    std::fs::create_dir_all(&folder).unwrap();
    let c = chat::start(&f.worlds, &folder).unwrap();
    let id = c["id"].as_str().unwrap().to_string();
    chat::say(&f.worlds, &id, "bonjour").unwrap();
    // fennec stops here; the next start recovers.
    chat::recover(&f.worlds).unwrap();
    let c = chat::load(&f.worlds, &id).unwrap();
    assert_eq!(c["status"], "idle");
    assert!(c["error"].as_str().unwrap().contains("interrupted"));
    chat::say(&f.worlds, &id, "encore").unwrap();
}

#[test]
fn a_folder_taken_back_between_two_calls_of_one_reply_closes_for_the_second() {
    let f = Fixture::new_in("chat-ungrant-calls", &var_tmp());
    let folder = f.worlds.home.join("notes");
    std::fs::create_dir_all(&folder).unwrap();
    let other = f.worlds.home.join("other");
    let third = f.worlds.home.join("third");
    for d in [&other, &third] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::write(d.join("o.txt"), "omega\n").unwrap();
    }
    let c = chat::start(&f.worlds, &folder).unwrap();
    let id = c["id"].as_str().unwrap().to_string();
    chat::write(&f.worlds, &id, true).unwrap();
    // A first turn opens `other`.
    chat::say(&f.worlds, &id, "lis ../other/o.txt").unwrap();
    let model = Replies(Mutex::new(VecDeque::from([
        call("c0", "shell", json!({"command": "cat ../other/o.txt"})),
        json!({"content": "ok."}),
    ])));
    let (w, tid) = (twin(&f), id.clone());
    let t = std::thread::spawn(move || chat::answer(&w, &tid, &model, 10));
    until_waiting(&f, &id);
    chat::decide(&f.worlds, &id, true).unwrap();
    t.join().unwrap().unwrap();
    // One reply, two calls: the first asks for `third`; while it waits, the
    // person takes `other` back, so the second call asks again.
    chat::say(&f.worlds, &id, "lis third, ecris other").unwrap();
    let mut two = call("c1", "shell", json!({"command": "cat ../third/o.txt"}));
    two["tool_calls"].as_array_mut().unwrap().push(
        call(
            "c2",
            "write_file",
            json!({"path": "../other/n.txt", "content": "x\n"}),
        )["tool_calls"][0]
            .clone(),
    );
    let model = Replies(Mutex::new(VecDeque::from([two, json!({"content": "ok."})])));
    let (w, tid) = (twin(&f), id.clone());
    let t = std::thread::spawn(move || chat::answer(&w, &tid, &model, 10));
    until_waiting(&f, &id);
    chat::ungrant(&f.worlds, &id, &other.display().to_string()).unwrap();
    chat::decide(&f.worlds, &id, true).unwrap();
    let pending = until_waiting(&f, &id);
    assert_eq!(pending["pending"]["directory"], other.display().to_string());
    chat::decide(&f.worlds, &id, false).unwrap();
    t.join().unwrap().unwrap();
    assert!(
        !other.join("n.txt").exists(),
        "a folder taken back stays closed"
    );
}

#[test]
fn an_approved_plan_allows_changes_within_the_turn() {
    let f = Fixture::new_in("chat-plan", &var_tmp());
    let folder = f.worlds.home.join("notes");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.txt"), "alpha\n").unwrap();
    let c = chat::start(&f.worlds, &folder).unwrap();
    let id = c["id"].as_str().unwrap().to_string();
    chat::say(&f.worlds, &id, "Copie a.txt dans b.txt").unwrap();
    let model = Replies(Mutex::new(VecDeque::from([
        call(
            "c0",
            "propose_plan",
            json!({"plan": "- copier a.txt", "steps": ["Copier a.txt"]}),
        ),
        call("c1", "shell", json!({"command": "cp a.txt b.txt"})),
        // The step is left open: the turn goes back to it.
        json!({"content": "C'est fait."}),
        call("c2", "criteria", json!({"action": "done", "number": 1})),
        json!({"content": "C'est fait."}),
    ])));
    let (w, tid) = (twin(&f), id.clone());
    let t = std::thread::spawn(move || chat::answer(&w, &tid, &model, 10));
    let pending = until_waiting(&f, &id);
    assert_eq!(pending["pending"]["plan"], "- copier a.txt");
    assert_eq!(pending["pending"]["steps"], json!(["Copier a.txt"]));
    chat::decide(&f.worlds, &id, true).unwrap();
    t.join().unwrap().unwrap();
    let c = chat::load(&f.worlds, &id).unwrap();
    assert!(c["error"].is_null(), "{}", c["error"]);
    assert_eq!(c["write"], true);
    assert!(c["grants"].as_array().unwrap().is_empty());
    assert!(folder.join("b.txt").exists(), "approved: the turn writes");
    assert_eq!(c["criteria"][0]["text"], "Copier a.txt");
    assert_eq!(c["criteria"][0]["state"], "done");
    let sent_back = sent_back(&c).len();
    assert_eq!(
        sent_back, 1,
        "a reply with the plan unfinished goes back to it"
    );
}

#[test]
fn the_plan_checks_run_after_a_change_and_failures_go_back() {
    let f = Fixture::new_in("chat-checks", &var_tmp());
    let folder = f.worlds.home.join("notes");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.txt"), "alpha\n").unwrap();
    let c = chat::start(&f.worlds, &folder).unwrap();
    let id = c["id"].as_str().unwrap().to_string();
    chat::say(&f.worlds, &id, "Copie a.txt dans b.txt").unwrap();
    let model = Replies(Mutex::new(VecDeque::from([
        call(
            "c0",
            "propose_plan",
            json!({"plan": "- copier a.txt", "checks": ["test -f b.txt"]}),
        ),
        call("c1", "shell", json!({"command": "touch c.txt"})),
        // The check fails: the reply goes back to work.
        json!({"content": "C'est fait."}),
        call("c2", "shell", json!({"command": "cp a.txt b.txt"})),
        json!({"content": "C'est fait."}),
    ])));
    let (w, tid) = (twin(&f), id.clone());
    let t = std::thread::spawn(move || chat::answer(&w, &tid, &model, 10));
    let pending = until_waiting(&f, &id);
    assert_eq!(pending["pending"]["checks"], json!(["test -f b.txt"]));
    chat::decide(&f.worlds, &id, true).unwrap();
    t.join().unwrap().unwrap();
    let c = chat::load(&f.worlds, &id).unwrap();
    assert!(c["error"].is_null(), "{}", c["error"]);
    assert_eq!(c["checks"]["results"][0]["command"], "test -f b.txt");
    assert_eq!(c["checks"]["results"][0]["exit"], 0);
    let sent_back = sent_back(&c);
    assert_eq!(sent_back.len(), 1, "{sent_back:?}");
    assert!(
        sent_back[0]["content"]
            .as_str()
            .unwrap()
            .contains("test -f b.txt")
    );
}

#[test]
fn a_message_said_again_drops_what_followed_it() {
    let f = Fixture::new("chat-replay");
    let folder = f.worlds.home.join("notes");
    std::fs::create_dir_all(&folder).unwrap();
    let c = chat::start(&f.worlds, &folder).unwrap();
    let id = c["id"].as_str().unwrap();
    for (said, reply) in [("un", "1"), ("deux", "2")] {
        chat::say(&f.worlds, id, said).unwrap();
        let model = Replies(Mutex::new(VecDeque::from([json!({"content": reply})])));
        chat::answer(&f.worlds, id, &model, 10).unwrap();
    }
    // Only a message of the person's can be said again.
    assert!(chat::replay(&f.worlds, id, 1, "x").is_err());
    chat::replay(&f.worlds, id, 0, "trois").unwrap();
    let model = Replies(Mutex::new(VecDeque::from([json!({"content": "3"})])));
    chat::answer(&f.worlds, id, &model, 10).unwrap();
    let c = chat::load(&f.worlds, id).unwrap();
    let shown = said(&c);
    let said: Vec<&str> = shown
        .iter()
        .map(|m| m["content"].as_str().unwrap())
        .collect();
    assert_eq!(said, ["trois", "3"]);
    // The cost spent before stays counted.
    assert_eq!(c["turns"].as_array().unwrap().len(), 3);
}

#[test]
fn a_task_conversation_reads_asks_then_hands_its_brief() {
    use fennec_os::spec::{Discussed, discuss};
    let f = Fixture::new("chat-task");
    let folder = f.worlds.home.join("proj");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.txt"), "alpha\n").unwrap();
    let id = chat::start(&f.worlds, &folder).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let mut thread = vec![json!({"who": "human", "text": "copie a.txt"})];
    // It reads what the request touches, then asks.
    let model = Replies(Mutex::new(VecDeque::from([
        call("c0", "read_file", json!({"path": "a.txt"})),
        json!({"content": "Vers quel fichier ?"}),
    ])));
    let (said, _) = discuss(&f.worlds, &id, &thread, &model).unwrap();
    assert_eq!(said, Discussed::Reply("Vers quel fichier ?".into()));
    let first = chat::load(&f.worlds, &id).unwrap()["messages"]
        .as_array()
        .unwrap()
        .clone();
    assert!(
        first
            .iter()
            .any(|m| m["role"] == "tool" && m["content"].as_str().unwrap().contains("alpha"))
    );
    // The answer goes on in the same conversation, which ends on `brief`.
    thread.push(json!({"who": "fennec", "text": "Vers quel fichier ?"}));
    thread.push(json!({"who": "human", "text": "b.txt"}));
    let model = Replies(Mutex::new(VecDeque::from([call(
        "c1",
        "brief",
        json!({"brief": "copier a.txt dans b.txt", "title": "Copier a.txt", "skills": ["nope"]}),
    )])));
    let (said, exchange) = discuss(&f.worlds, &id, &thread, &model).unwrap();
    // An unknown skill is never picked; which skills exist depends on the machine.
    let Discussed::Brief(brief, skills, roles, title) = said else {
        panic!("{said:?}")
    };
    assert_eq!(brief, "copier a.txt dans b.txt");
    assert!(skills.unwrap_or_default().is_empty());
    assert_eq!(roles, None);
    assert_eq!(title.as_deref(), Some("Copier a.txt"));
    assert_eq!(
        exchange["turns"].as_array().unwrap().len(),
        1,
        "this turn's cost only"
    );
    let c = chat::load(&f.worlds, &id).unwrap();
    let now = c["messages"].as_array().unwrap();
    assert_eq!(now[..first.len()], first[..], "the conversation went on");
    assert!(!folder.join("b.txt").exists(), "read-only");
    assert!(
        chat::write(&f.worlds, &id, true).is_err(),
        "a task's never writes"
    );
    // The task reworded: the conversation starts over from its record.
    let model = Replies(Mutex::new(VecDeque::from([
        json!({"content": "D'accord ?"}),
    ])));
    discuss(
        &f.worlds,
        &id,
        &[json!({"who": "human", "text": "autre chose"})],
        &model,
    )
    .unwrap();
    let shown = said_of(&chat::load(&f.worlds, &id).unwrap());
    assert_eq!(shown[0]["content"], "autre chose");
}

fn said_of(c: &Value) -> Vec<Value> {
    said(c)
}

#[test]
fn an_analysis_reads_then_ends_on_its_report_and_never_on_a_brief() {
    use fennec_os::analysis::{Analysed, analyse};
    let f = Fixture::new("chat-analysis");
    let folder = f.worlds.home.join("proj");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.txt"), "alpha\n").unwrap();
    let id = chat::start(&f.worlds, &folder).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    chat::link(&f.worlds, &id, "task1").unwrap();
    assert!(chat::analyse_as(&f.worlds, &id, "guess").is_err());
    chat::analyse_as(&f.worlds, &id, "audit").unwrap();
    let thread = vec![json!({"who": "human", "text": "audite"})];
    let model = Replies(Mutex::new(VecDeque::from([
        call("c0", "read_file", json!({"path": "a.txt"})),
        // A brief is not this conversation's end: it is refused as unknown.
        call("c1", "brief", json!({"brief": "x", "title": "x"})),
        call(
            "c2",
            "report",
            json!({"summary": "un seul fichier",
                   "suggestions": [{"title": "ajouter b.txt", "kind": "task", "task": "ajoute b.txt"}],
                   "proposals": [{"file": "ARCHITECTURE.md", "content": "# vision\n"}]}),
        ),
    ])));
    let (said, exchange) = analyse(&f.worlds, &id, &thread, &model).unwrap();
    let Analysed::Report(r) = said else {
        panic!("no report")
    };
    assert_eq!(r["summary"], "un seul fichier");
    assert_eq!(r["suggestions"][0]["kind"], "task");
    assert_eq!(r["proposals"][0]["file"], "ARCHITECTURE.md");
    assert!(exchange["turns"].as_array().unwrap().len() >= 3);
    let c = chat::load(&f.worlds, &id).unwrap();
    assert!(c["brief"].is_null(), "an analysis never briefs");
    assert!(!folder.join("ARCHITECTURE.md").exists(), "nothing written");
    // The next turn starts without the old report: a question back is a reply.
    let model = Replies(Mutex::new(VecDeque::from([
        json!({"content": "Quel module ?"}),
    ])));
    let thread = vec![
        json!({"who": "human", "text": "audite"}),
        json!({"who": "human", "text": "encore"}),
    ];
    let (said, _) = analyse(&f.worlds, &id, &thread, &model).unwrap();
    assert!(matches!(said, Analysed::Reply(t) if t == "Quel module ?"));
}
