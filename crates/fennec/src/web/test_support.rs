//! The `App` the web adapter's tests build: a real store on disk, no turn running.

use crate::config::{LlmSettings, Settings};
use crate::domain::preferences::ModelParams;
use crate::notifier::Notifier;
use crate::web::App;
use crate::web::turns::Turns;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// A file-backed store: `App::store` opens one connection per call, so an
/// in-memory DB would be empty on every call.
pub(super) fn app(db_path: PathBuf) -> App {
    let settings = Settings {
        llm: LlmSettings {
            base_url: Some("http://localhost".into()),
            api_key: None,
            defaults: ModelParams {
                provider: "local".into(),
                model: "test".into(),
                max_tokens: 100,
                context_window: 1000,
                compact_at: 0.85,
                reasoning_effort: None,
            },
            listed: Vec::new(),
        },
        db_path,
    };
    let notifier = Arc::new(Notifier::default());
    App {
        turns: Turns::new(settings.clone(), notifier.clone()),
        settings,
        port: 0,
        listen: std::net::Ipv4Addr::LOCALHOST.into(),
        logins: crate::web::login::Logins::default(),
        notifier: notifier.clone(),
        skill_folds: Mutex::new(HashMap::new()),
        idea_reports: Mutex::new(HashMap::new()),
        idea_queues: Mutex::default(),
        worktree_scan: Mutex::default(),
        server_runs: Arc::new(Mutex::new(HashMap::new())),
        plugins: Arc::new(crate::plugins::PluginManager::new(
            crate::sandbox::Sandbox::Off,
            std::env::temp_dir(),
            std::env::temp_dir(),
            notifier.clone(),
        )),
        agents_home: std::env::temp_dir(),
        update_check: Mutex::default(),
        listings: Default::default(),
    }
}

/// The full response a route writes: the route runs on the server end of a
/// real socket, which is closed before the client reads to the end.
pub(super) fn response(route: impl FnOnce(&mut std::net::TcpStream)) -> String {
    use std::io::Read;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let mut server = listener.accept().unwrap().0;
    route(&mut server);
    drop(server);
    let mut raw = Vec::new();
    client.read_to_end(&mut raw).unwrap();
    String::from_utf8_lossy(&raw).into_owned()
}

/// The value of a response header, matched case-insensitively.
pub(super) fn header(response: &str, name: &str) -> Option<String> {
    response
        .split("\r\n\r\n")
        .next()?
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(n, _)| n.trim().eq_ignore_ascii_case(name))
        .map(|(_, v)| v.trim().to_string())
}
