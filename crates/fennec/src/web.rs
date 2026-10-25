//! Web adapter: the local server behind the UI. std only, one thread per
//! connection, bound to 127.0.0.1.

mod assets;
mod event_stream;
mod guard;
mod http;
mod notifier;
mod routes;
mod turns;

use crate::config::Settings;
use crate::store::SqliteStore;
use notifier::Notifier;
use serde_json::json;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use turns::Turns;

/// What every request handler shares.
struct App {
    settings: Settings,
    port: u16,
    turns: Turns,
    notifier: Arc<Notifier>,
}

impl App {
    /// A connection per request: SQLite connections are not shared across
    /// threads.
    fn store(&self) -> Result<SqliteStore, String> {
        Ok(SqliteStore::open(&self.settings.db_path)?)
    }
}

pub fn serve(port: u16, settings: Settings) -> Result<(), String> {
    let listener =
        TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("127.0.0.1:{port}: {e}"))?;
    eprintln!("fennec · {} · http://127.0.0.1:{port}", settings.llm.model);
    let notifier = Arc::new(Notifier::default());
    let app = Arc::new(App {
        turns: Turns::new(settings.clone(), notifier.clone()),
        settings,
        port,
        notifier,
    });
    // Fail now rather than on the first request.
    app.store()?;
    for stream in listener.incoming().flatten() {
        let app = app.clone();
        std::thread::spawn(move || {
            let mut stream = stream;
            if let Err(e) = handle(&app, &mut stream) {
                let _ = http::send_json(&mut stream, 400, &json!({ "error": e }));
            }
        });
    }
    Ok(())
}

fn handle(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    let req = http::read_request(stream)?;
    if let Err(refusal) = guard::check(&req, app.port) {
        return http::error(stream, refusal.status, refusal.message);
    }
    routes::handle(app, &req, stream)
}
