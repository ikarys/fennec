mod common;

use std::collections::VecDeque;
use std::io;
use std::sync::Mutex;

use common::Fixture;
use fennec_os::chat;
use fennec_os::provider::Model;
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
