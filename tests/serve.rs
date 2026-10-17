mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use common::Fixture;
use fennec_os::review;
use fennec_os::world::Worlds;

fn http(port: u16, host: &str, request: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let request = request.replace("HOST", host);
    s.write_all(request.as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}

#[test]
fn the_review_page_obeys_only_its_human_and_keeps_diffs_inert() {
    let fx = Fixture::new("serve");
    let runs = fx.worlds.root.join("runs");
    std::fs::create_dir_all(&runs).unwrap();
    std::fs::write(
        runs.join("abc123.json"),
        r#"{"project": "/p", "goal": "g", "status": "ready", "winner": "abc123",
            "attempts": [{"world": "abc123", "passed": true, "diff": "+<script>x()</script>"}]}"#,
    )
    .unwrap();

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let host = format!("127.0.0.1:{port}");
    let worlds = Worlds {
        root: fx.worlds.root.clone(),
        home: fx.worlds.home.clone(),
        ro_paths: Vec::new(),
        memory_mb: fx.worlds.memory_mb,
    };
    std::thread::spawn(move || fennec_os::serve::serve(worlds, listener, "s3cret"));

    let get = |path: &str, cookie: &str| {
        http(
            port,
            &host,
            &format!("GET {path} HTTP/1.1\r\nHost: HOST\r\nCookie: {cookie}\r\n\r\n"),
        )
    };
    // No cookie, a wrong one, or another Host (DNS rebinding): refused.
    assert!(get("/", "").starts_with("HTTP/1.1 403"));
    assert!(get("/", "fennec-os=guess").starts_with("HTTP/1.1 403"));
    let rebound = http(
        port,
        "evil.example",
        "GET /?t=s3cret HTTP/1.1\r\nHost: HOST\r\n\r\n",
    );
    assert!(rebound.starts_with("HTTP/1.1 403"));

    // The printed secret buys a strict cookie.
    let login = get("/?t=s3cret", "");
    assert!(login.starts_with("HTTP/1.1 303"), "{login}");
    assert!(login.contains("Set-Cookie: fennec-os=s3cret; HttpOnly; SameSite=Strict"));
    let cookie = "fennec-os=s3cret";

    // An agent's diff is shown, never run.
    let page = get("/run/abc123", cookie);
    assert!(page.starts_with("HTTP/1.1 200"), "{page}");
    assert!(page.contains("&lt;script&gt;x()&lt;/script&gt;"));
    assert!(!page.contains("<script"));
    assert!(page.contains("Content-Security-Policy: default-src 'none'"));
    assert!(get("/run/..", cookie).starts_with("HTTP/1.1 404"));

    // A decision from the page is the CLI's decision.
    let body = "correction=test+the+%22empty%22+case";
    let post = http(
        port,
        &host,
        &format!(
            "POST /run/abc123/reject HTTP/1.1\r\nHost: HOST\r\nCookie: {cookie}\r\n\
             Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    );
    assert!(post.starts_with("HTTP/1.1 303"), "{post}");
    for _ in 0..100 {
        if review::load(&fx.worlds, "abc123").unwrap()["status"] == "rejected" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let run = review::load(&fx.worlds, "abc123").unwrap();
    assert_eq!(run["status"], "rejected");
    assert_eq!(run["correction"], "test the \"empty\" case");
}
