//! Turns started from the UI: each runs on its own thread, reports through
//! the event log, and can be stopped. One turn at a time per session.

use super::children::Children;
use super::model_routes;
use super::notifier::{Notifier, NotifyingLog};
use crate::agent::turn::{Turn, compact_session, replay_turn, run_turn};
use crate::agent::workflow::{WorkflowEnv, WorkflowJob, run_workflow};
use crate::agents;
use crate::config::Settings;
use crate::domain::attachment::Attachment;
use crate::domain::event::Event;
use crate::domain::slash::TurnInput;
use crate::instructions::all_instructions;
use crate::llm::OpenAiClient;
use crate::metering::MeteredLog;
use crate::protected;
use crate::skills;
use crate::store::SqliteStore;
use crate::tools::{LocalTools, Sandbox};
use crate::worktree::{self, Worktree};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Where a session's turns run: its worktree when it has one, the project's
/// directory otherwise.
pub struct Place {
    pub workdir: PathBuf,
    pub worktree: Option<Worktree>,
}

impl Place {
    pub fn new(project_dir: &str, worktree: Option<Worktree>) -> Place {
        Place {
            workdir: worktree
                .as_ref()
                .map_or_else(|| PathBuf::from(project_dir), |wt| wt.workdir.clone()),
            worktree,
        }
    }
}

/// What a turn started from the UI does.
pub enum Job {
    /// A message typed in the composer (or the answer to a question).
    Input(TurnInput, Vec<Attachment>),
    /// The human replays one of their messages.
    Replay(String),
    /// A workflow run (start, answer, continue, resume, stop).
    Workflow(WorkflowJob),
}

pub struct Turns {
    settings: Settings,
    notifier: Arc<Notifier>,
    /// Sessions with a turn running, and the flag that stops it. Shared with
    /// each turn's thread, which removes its entry when it ends.
    running: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl Turns {
    pub fn new(settings: Settings, notifier: Arc<Notifier>) -> Turns {
        Turns {
            settings,
            notifier,
            running: Arc::default(),
        }
    }

    pub fn is_running(&self, session_id: &str) -> bool {
        self.running.lock().unwrap().contains_key(session_id)
    }

    pub fn stop(&self, session_id: &str) {
        if let Some(flag) = self.running.lock().unwrap().get(session_id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Starts a turn; refuses while one runs on the session.
    pub fn start(&self, session_id: &str, place: Place, job: Job) -> Result<(), String> {
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut running = self.running.lock().unwrap();
            if running.contains_key(session_id) {
                return Err("a turn is already running".into());
            }
            running.insert(session_id.to_string(), stop.clone());
        }
        let settings = self.settings.clone();
        let notifier = self.notifier.clone();
        let running = self.running.clone();
        let session_id = session_id.to_string();
        std::thread::spawn(move || {
            if let Err(e) = run(
                &settings,
                &notifier,
                &session_id,
                &place.workdir,
                &job,
                &stop,
            ) {
                eprintln!("fennec: session {session_id}: {e}");
            }
            // Whatever the job did, what the agent wrote is recorded as
            // fennec's commits for the human to cherry-pick.
            if let Some(wt) = &place.worktree {
                let message = format!("fennec: agent work in session {session_id}");
                if let Err(e) = worktree::commit(wt, &message) {
                    eprintln!("fennec: session {session_id}: commit: {e}");
                }
            }
            running.lock().unwrap().remove(&session_id);
            // Whoever waits for the turn to end (the ideas queue) wakes now.
            notifier.notify();
        });
        Ok(())
    }
}

/// Builds the turn's dependencies and hands the turn (and its tools) to
/// `body`. The store, log, prefs, instructions, skills, agents, model and
/// tools stay locals here.
pub(super) fn with_turn<R>(
    settings: &Settings,
    notifier: &Notifier,
    session_id: &str,
    workdir: &Path,
    stop: &AtomicBool,
    body: impl FnOnce(&mut Turn, &LocalTools) -> Result<R, String>,
) -> Result<R, String> {
    let store = SqliteStore::open(&settings.db_path)?;
    let ledger = token_usage::Ledger::open(&settings.usage_path()).map_err(|e| e.to_string())?;
    let metered = MeteredLog {
        log: &store,
        store: &store,
        ledger: &ledger,
    };
    let log = NotifyingLog {
        log: &metered,
        notifier,
    };
    let prefs = store.preferences()?;
    // A workflow's child sessions run on the model the human chose for the session.
    let (chosen, _) = model_routes::session_choice(&store, session_id)?;
    let params = settings.llm.for_turn(&prefs, chosen.as_ref());
    let instructions = all_instructions(prefs.global_instructions.as_deref(), workdir);
    let skills = skills::discover(workdir);
    let agents = agents::discover(workdir);
    let model = OpenAiClient::new(settings.endpoint(&prefs, &params.provider)?, &params.model);
    let protection = protected::load(workdir);
    let tools = LocalTools::new(workdir.to_path_buf())
        .with_search(prefs.search.clone())
        .with_sandbox(
            Sandbox::for_store(&prefs.sandbox, &settings.db_path)
                .protecting(protected::existing_paths(workdir, &protection)),
        )
        .with_protection(protection);
    let mut on_event = |_: &Event| {};
    let mut turn = Turn {
        model: &model,
        log: &log,
        tools: &tools,
        session_id,
        workdir,
        instructions: &instructions,
        skills: &skills,
        always_on: &prefs.always_on_skills,
        agents: &agents,
        max_tokens: params.max_tokens,
        context_window: params.context_window,
        compact_at: params.compact_at,
        reasoning_effort: params.reasoning_effort,
        stop,
        on_event: &mut on_event,
    };
    body(&mut turn, &tools)
}

fn run(
    settings: &Settings,
    notifier: &Notifier,
    session_id: &str,
    workdir: &Path,
    job: &Job,
    stop: &AtomicBool,
) -> Result<(), String> {
    with_turn(
        settings,
        notifier,
        session_id,
        workdir,
        stop,
        |turn, tools| match job {
            Job::Input(TurnInput::Prompt(prompt), attachments) => {
                run_turn(turn, prompt, attachments).map(|_| ())
            }
            Job::Input(TurnInput::Compact, _) => compact_session(turn).map(|_| ()),
            Job::Replay(message_id) => replay_turn(turn, message_id).map(|_| ()),
            Job::Workflow(workflow_job) => {
                let children = Children {
                    settings,
                    notifier,
                    workdir,
                };
                run_workflow(
                    turn,
                    &WorkflowEnv {
                        children: &children,
                        commands: tools,
                    },
                    workflow_job.clone(),
                )
                .map(|_| ())
            }
        },
    )
}
