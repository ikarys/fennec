//! Web adapter: the server behind the UI. std only, one thread per connection,
//! bound to the configured address.

mod assets;
mod children;
mod guard;
mod http;
mod jobs;
mod listings;
mod login;
mod os;
mod routes;
mod streams;
#[cfg(test)]
mod test_support;
mod turns;
mod views;

use crate::agent::ports::EventLog;
use crate::chat;
use crate::config::Settings;
use crate::domain::project::{Project, SessionSummary};
use crate::domain::session_skills::{SessionSkills, fold_skills};
use crate::domain::skill::Skill;
use crate::notifier::Notifier;
use crate::sandbox::Sandbox;
use crate::store::SqliteStore;
use crate::web::http::HttpError;
use crate::worktree;
use os::servers::RunningServer;
use serde_json::json;
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use turns::{Place, Turns};

/// One thread per connection; 256 keeps an open SSE tab from exhausting the process.
const MAX_CONNECTIONS: usize = 256;

static ACTIVE_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

/// Drops the active-connection count when a handler thread ends, even if it panics.
struct ConnectionSlot;

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
    }
}

/// What every request handler shares.
struct App {
    settings: Settings,
    port: u16,
    /// The address this process bound, to tell the human when a changed `listen` needs a restart.
    pub listen: std::net::IpAddr,
    logins: login::Logins,
    turns: Turns,
    notifier: Arc<Notifier>,
    /// The skills fold per session, keyed by the log's max seq: the log is
    /// append-only, so an unchanged max seq means an unchanged fold. The fold
    /// is O(log) and the UI refetches it on every relevant event; caching keeps
    /// the steady state to a max-seq lookup.
    skill_folds: Mutex<HashMap<String, (u64, SessionSkills)>>,
    /// Each idea session's board report, keyed by its log's max seq like
    /// `skill_folds`: the UI polls the board while a run goes on.
    idea_reports: Mutex<HashMap<String, (u64, views::idea_board::IdeaReport)>>,
    /// Projects whose ideas queue runs, and the flag that stops it after the
    /// current idea (`jobs::idea_queue::start_queue`).
    idea_queues: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// The last worktrees disk scan and when it ran (ms): a scan walks gigabytes,
    /// so the home page's total reuses it for a while (`worktree_routes`).
    worktree_scan: Mutex<Option<(u64, Vec<crate::worktree::usage::WorktreeUsage>)>>,
    /// Servers running per (session, name): the pid of each process group.
    server_runs: Arc<Mutex<HashMap<String, RunningServer>>>,
    /// The active plugin hosts and the event pump that feeds them.
    plugins: Arc<crate::plugins::PluginManager>,
    /// The agents home (`<home>/.agents`): where the global plugin
    /// definitions live.
    agents_home: std::path::PathBuf,
    /// The last release check and when it ran: GitHub is asked at most every
    /// few hours (`update_routes`).
    update_check: Mutex<Option<(std::time::Instant, crate::update::Check)>>,
    /// The last model listing each endpoint answered: the Settings page shows it at once
    /// and re-lists in the background, so a busy model server never delays the page.
    listings: listings::Listings,
}

impl App {
    /// A connection per request: SQLite connections are not shared across
    /// threads.
    fn store(&self) -> Result<SqliteStore, String> {
        Ok(SqliteStore::open(&self.settings.db_path)?)
    }

    /// The session and its project, or `None` when there is no such session.
    /// Shared by the route modules, which depend on this, not on `routes`.
    fn session_and_project(&self, id: &str) -> Result<Option<(SessionSummary, Project)>, String> {
        let store = self.store()?;
        let Some(s) = store.session(id)? else {
            return Ok(None);
        };
        Ok(store.project(&s.project_id)?.map(|p| (s, p)))
    }

    /// The session and its project, or a 404 for the client.
    fn require_session(&self, id: &str) -> Result<(SessionSummary, Project), HttpError> {
        self.session_and_project(id)?
            .ok_or_else(|| HttpError::new(404, "no such session"))
    }

    /// Where the session's turns run (see `Place`). A child session shares
    /// its root session's worktree — or, for the chat, its root session's
    /// scratch directory.
    fn place(
        &self,
        store: &SqliteStore,
        session: &SessionSummary,
        project: &Project,
    ) -> Result<Place, String> {
        let root = self.root_id(store, session)?;
        if chat::is_chat(project) {
            return Ok(Place::new(
                &chat::session_dir(&self.settings.db_path, &root)
                    .display()
                    .to_string(),
                None,
            ));
        }
        let found = worktree::find(&self.worktrees(), Path::new(&project.workdir), &root);
        Ok(Place::new(&project.workdir, found))
    }

    /// Where a workflow launch runs: with `worktree`, like `place` but the
    /// root session's worktree is created on first call (`None` outside a git
    /// repository: it then runs in place); without, the human chose their own
    /// folder. An existing worktree is kept either way: the choice applies
    /// when the session has none.
    fn open_place(
        &self,
        store: &SqliteStore,
        session: &SessionSummary,
        project: &Project,
        worktree: bool,
    ) -> Result<Place, String> {
        if !worktree {
            return self.place(store, session, project);
        }
        if chat::is_chat(project) {
            // The chat's scratch is not a git repository: no worktree.
            return self.place(store, session, project);
        }
        let root = self.root_id(store, session)?;
        let opened = worktree::open(&self.worktrees(), Path::new(&project.workdir), &root)?;
        Ok(Place::new(&project.workdir, opened))
    }

    fn worktrees(&self) -> std::path::PathBuf {
        worktree::root(&self.settings.db_path)
    }

    /// The top of the session's `parent_id` chain.
    fn root_id(&self, store: &SqliteStore, session: &SessionSummary) -> Result<String, String> {
        let mut id = session.id.clone();
        let mut parent = session.parent_id.clone();
        while let Some(parent_id) = parent {
            let s = store
                .session(&parent_id)?
                .ok_or_else(|| format!("no parent session {parent_id}"))?;
            (id, parent) = (s.id, s.parent_id);
        }
        Ok(id)
    }

    /// The session's skills fold, cached against the log's max seq (see
    /// `skill_folds`): the O(log) re-read runs only when a new event has
    /// followed. `skills` is the project's registry, discovered by the caller
    /// on every request: a cache hit saves the log re-read, not the discovery,
    /// and the key ignores the registry (a skill added or removed shows on the
    /// next event).
    fn folded_skills(
        &self,
        store: &SqliteStore,
        id: &str,
        skills: &[Skill],
    ) -> Result<SessionSkills, String> {
        let last = store.last_seq(id)?;
        let mut cache = crate::locks::lock(&self.skill_folds);
        if cache.get(id).is_some_and(|(seq, _)| *seq == last) {
            return Ok(cache[id].1.clone());
        }
        let folded = fold_skills(&store.events(id)?, skills);
        cache.insert(id.to_string(), (last, folded.clone()));
        Ok(folded)
    }

    /// The session's idea report, cached against the log's max seq (see
    /// `idea_reports`): the O(log) re-read runs only when a new event has
    /// followed.
    pub(crate) fn idea_report(
        &self,
        store: &SqliteStore,
        session_id: &str,
    ) -> Result<views::idea_board::IdeaReport, String> {
        let last = store.last_seq(session_id)?;
        let mut cache = crate::locks::lock(&self.idea_reports);
        if cache.get(session_id).is_some_and(|(seq, _)| *seq == last) {
            return Ok(cache[session_id].1.clone());
        }
        let report = views::idea_board::idea_report(&store.events(session_id)?);
        cache.insert(session_id.to_string(), (last, report.clone()));
        Ok(report)
    }
}

pub fn serve(
    port: u16,
    listen: std::net::IpAddr,
    password: Option<String>,
    settings: Settings,
) -> Result<(), String> {
    crate::domain::server_config::check_listen(listen, password.is_some())
        .map_err(|message| format!("settings.yaml: {message}"))?;
    let listener = TcpListener::bind((listen, port))
        .map_err(|e| format!("{}: {e}", std::net::SocketAddr::new(listen, port)))?;
    crate::sandbox::set_server_port(port);
    let notifier = Arc::new(Notifier::default());
    // The agents home (`<home>/.agents`), resolved the way skills and agents
    // resolve it: the plugins live only there, never in a project.
    let agents_home = std::env::var_os("HOME")
        .map(|home| std::path::PathBuf::from(home).join(".agents"))
        .unwrap_or_else(|| std::path::PathBuf::from(".agents"));
    // The plugin manager's sandbox world and scratch root, from the stored
    // preferences. A plugin's network is its own choice, set per activation.
    let boot_store = SqliteStore::open(&settings.db_path)?;
    let prefs = boot_store.preferences().map_err(|e| e.to_string())?;
    let model = settings.llm.for_turn(&prefs, None).model;
    eprintln!(
        "fennec · {} · http://{}",
        if model.is_empty() {
            "no model yet, finish the setup"
        } else {
            &model
        },
        std::net::SocketAddr::new(listen, port)
    );
    let prefs = boot_store.preferences()?;
    let plugins_temp = settings
        .db_path
        .parent()
        .map(|dir| dir.join("plugins"))
        .unwrap_or_else(|| std::path::PathBuf::from("plugins"));
    let plugins = Arc::new(crate::plugins::PluginManager::new(
        Sandbox::for_store(&prefs.sandbox, &settings.db_path),
        agents_home.clone(),
        plugins_temp,
        notifier.clone(),
    ));
    let logins = login::Logins::new(password);
    let app = Arc::new(App {
        turns: Turns::new(settings.clone(), notifier.clone()),
        settings,
        port,
        listen,
        logins,
        notifier: notifier.clone(),
        skill_folds: Mutex::new(HashMap::new()),
        idea_reports: Mutex::new(HashMap::new()),
        idea_queues: Mutex::default(),
        worktree_scan: Mutex::default(),
        server_runs: Arc::new(Mutex::new(HashMap::new())),
        plugins: plugins.clone(),
        agents_home,
        update_check: Mutex::default(),
        listings: Default::default(),
    });
    // The event pump feeds the active plugins; it reads the store only.
    app.plugins
        .start_pump(app.plugins.clone(), &app.settings.db_path, &app.notifier);
    // Fail now rather than on the first request. No turn runs yet, so a log
    // still marked running was cut by a crash.
    let store = app.store()?;
    // The chat's scratch project: a session with no repository. Its
    // per-session scratch directory is created only when a tool first needs
    // it, so starting the server creates nothing on disk. Non-fatal: a
    // machine that cannot register it still serves its projects.
    let chat_dir = chat::workdir(&app.settings.db_path);
    let chat_init = store
        .chat_project(&chat_dir.display().to_string())
        .map_err(|e| e.to_string());
    if let Err(e) = chat_init {
        eprintln!("fennec: chat project: {e}");
    }
    // Idempotent: calls recorded before the ledger existed join it once.
    let ledger =
        token_usage::Ledger::open(&app.settings.usage_path()).map_err(|e| e.to_string())?;
    let imported = crate::metering::backfill(&store, &ledger)?;
    if imported > 0 {
        eprintln!(
            "fennec · {imported} past model call(s) added to or labelled in the usage ledger"
        );
    }
    let closed = store.close_dangling_turns()?;
    if closed > 0 {
        eprintln!("fennec · closed {closed} turn(s) left open by a crash");
    }
    let resumed_queues = jobs::idea_queue::resume_interrupted_runs(&app)?;
    if resumed_queues > 0 {
        eprintln!("fennec · resumed {resumed_queues} ideas queue(s) cut by a restart");
    }
    for stream in listener.incoming().flatten() {
        let mut stream = stream;
        if ACTIVE_CONNECTIONS.fetch_add(1, Ordering::Relaxed) + 1 > MAX_CONNECTIONS {
            ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
            let _ = http::send_json(
                &mut stream,
                503,
                &json!({ "error": "too many connections" }),
            );
            continue;
        }
        let app = app.clone();
        let slot = ConnectionSlot;
        std::thread::spawn(move || {
            let _slot = slot;
            let mut stream = stream;
            if let Err(e) = handle(&app, &mut stream) {
                if e.status >= 500 {
                    eprintln!("fennec: {} {}", e.status, e.message);
                }
                let _ = http::error(&mut stream, e.status, &e.message);
            }
        });
    }
    Ok(())
}

fn handle(app: &Arc<App>, stream: &mut TcpStream) -> Result<(), HttpError> {
    // A request that cannot be read is the client's fault, not the server's.
    let req = http::read_request(stream).map_err(|e| HttpError::new(400, e))?;
    // A sandboxed command shares the loopback when its network is on: it is never the human.
    let peer_is_loopback = match (stream.peer_addr(), stream.local_addr()) {
        (Ok(peer), Ok(server)) => {
            peer.ip().is_loopback() && !crate::sandbox::peer::is_sandboxed(peer, server)
        }
        _ => false,
    };
    let caller = match guard::check(&req, app.port, peer_is_loopback, app.logins.enabled()) {
        Ok(caller) => caller,
        Err(refusal) => return http::error(stream, refusal.status, refusal.message),
    };
    match caller {
        guard::Caller::Local => routes::handle(app, &req, stream),
        guard::Caller::Remote => login::gate(app, &req, stream),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::{Event, ToolCall};
    use crate::domain::skill::Skill;
    use crate::store::SqliteStore;
    use serde_json::json;
    use std::path::PathBuf;

    /// A minimal `App`; `folded_skills` takes the store, so its settings are
    /// unused here.
    fn app() -> App {
        app_with_db(PathBuf::from(":memory:"))
    }

    fn app_with_db(db_path: PathBuf) -> App {
        test_support::app(db_path)
    }

    fn skill(id: &str) -> Skill {
        Skill {
            id: id.into(),
            description: format!("{id} description"),
            prompt: "prompt".into(),
            directory: Some(PathBuf::from(id)),
        }
    }

    fn load_call(id: &str, skill_id: &str) -> Event {
        Event::ToolCall {
            message_id: "m".into(),
            tool_call: ToolCall {
                id: id.into(),
                name: "load_skill".into(),
                arguments: json!({"skillId": skill_id}),
                parse_error: None,
                raw_arguments: None,
            },
        }
    }

    fn load_result(id: &str) -> Event {
        Event::ToolResult {
            message_id: "m".into(),
            tool_call_id: id.into(),
            result: crate::domain::event::ToolResult::success("ok".into(), false),
        }
    }

    #[test]
    fn folded_skills_caches_by_watermark_and_invalidates_on_a_new_event() {
        let app = app();
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        let sid = store.create_session(&project, "s").unwrap();
        let skills = vec![skill("alpha"), skill("beta")];

        store.append(&sid, &load_call("c1", "alpha")).unwrap();
        store.append(&sid, &load_result("c1")).unwrap();

        let first = app.folded_skills(&store, &sid, &skills).unwrap();
        assert_eq!(first.loaded, vec!["alpha"]);

        // The log is unchanged, so the cache answers with the same fold.
        assert_eq!(app.folded_skills(&store, &sid, &skills).unwrap(), first);

        // A new event advances the watermark and the fold picks it up.
        store.append(&sid, &load_call("c2", "beta")).unwrap();
        store.append(&sid, &load_result("c2")).unwrap();
        let third = app.folded_skills(&store, &sid, &skills).unwrap();
        assert_eq!(third.loaded, vec!["alpha", "beta"]);
    }

    #[test]
    fn a_child_session_runs_in_its_roots_worktree() {
        let base =
            std::env::temp_dir().join(format!("fennec-place-{}", crate::domain::id::new_id()));
        let repo = base.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        for args in [
            &["init", "-q"][..],
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@e",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "init",
            ],
        ] {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .status();
            assert!(ok.unwrap().success());
        }
        let app = app_with_db(base.join("fennec.db"));
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for(repo.to_str().unwrap()).unwrap();
        let root_id = store.create_session(&project, "root").unwrap();
        let child_id = store.create_child_session(&root_id, "child").unwrap();
        let root = store.session(&root_id).unwrap().unwrap();
        let child = store.session(&child_id).unwrap().unwrap();

        let before = app.place(&store, &child, &project).unwrap();
        assert!(before.worktree.is_none());
        assert_eq!(before.workdir, repo);

        let opened = app.open_place(&store, &root, &project, true).unwrap();
        let wt = opened.worktree.clone().unwrap();
        assert_eq!(opened.workdir, wt.workdir);
        let from_child = app.place(&store, &child, &project).unwrap();
        assert_eq!(from_child.worktree, Some(wt));
        assert_eq!(from_child.workdir, opened.workdir);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_queue_cut_by_a_restart_starts_again() {
        let base =
            std::env::temp_dir().join(format!("fennec-requeue-{}", crate::domain::id::new_id()));
        std::fs::create_dir_all(&base).unwrap();
        let app = Arc::new(app_with_db(base.join("fennec.db")));
        let store = app.store().unwrap();
        let project = store.project_for("/tmp/requeue").unwrap();
        store
            .append_idea_event(
                &project.id,
                &backlog::BacklogEvent::QueueStarted { autonomous: true },
            )
            .unwrap();

        assert_eq!(jobs::idea_queue::resume_interrupted_runs(&app).unwrap(), 1);
        for _ in 0..200 {
            if !jobs::idea_queue::queue_running(&app, &project.id) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // The cut run is closed, then a new run in the same mode starts and,
        // with nothing ready, ends on its own.
        let run = store.backlog(&project.id).unwrap().last_run.unwrap();
        assert!(run.autonomous);
        assert_eq!(run.reason.as_deref(), Some("no idea ready"));
        let _ = std::fs::remove_dir_all(&base);
    }
}
