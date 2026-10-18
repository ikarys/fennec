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
    let messages = c["messages"].as_array().unwrap();
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

/// A second handle on the same store, for the thread the turn runs in.
fn twin(f: &Fixture) -> Worlds {
    Worlds {
        root: f.worlds.root.clone(),
        home: f.worlds.home.clone(),
        ro_paths: Vec::new(),
        memory_mb: Some(8192),
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
fn var_tmp() -> std::path::PathBuf {
    let p = std::path::PathBuf::from("/var/tmp");
    if p.is_dir() { p } else { std::env::temp_dir() }
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
    let refused = &c["messages"].as_array().unwrap()[2];
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
