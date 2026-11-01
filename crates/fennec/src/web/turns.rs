//! Turns started from the UI: each runs on its own thread, reports through
//! the event log, and can be stopped. One turn at a time per session.

use super::children::Children;
use super::model_routes;
use crate::agent::ports::EventLog;
use crate::agent::turn::{Turn, compact_session, replay_turn, run_turn};
use crate::agent::workflow::{WorkflowEnv, WorkflowJob, run_workflow};
use crate::config::Settings;
use crate::domain::attachment::Attachment;
use crate::domain::event::Event;
use crate::domain::queue;
use crate::domain::rule_approval::Choice;
use crate::domain::slash::{TurnInput, resolve};
use crate::notifier::{Notifier, NotifyingLog};
use crate::skills;
use crate::store::SqliteStore;
use crate::tools::LocalTools;
use crate::turn_build;
use crate::worktree::{self, Worktree};
use std::collections::{HashMap, HashSet};
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
    /// The human answered a rule approval.
    Approval {
        call_id: String,
        choice: Choice,
        rule_id: Option<i64>,
    },
    /// The human answered a waiver request.
    Waiver { call_id: String },
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

    /// A copy of the running map's keys, for the server-wide notice stream.
    pub fn running_ids(&self) -> HashSet<String> {
        self.running.lock().unwrap().keys().cloned().collect()
    }

    pub fn stop(&self, session_id: &str) {
        if let Some(flag) = self.running.lock().unwrap().get(session_id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Records a queued message while the session's turn is running. The
    /// running lock is held while `add` appends, so a turn that ends in
    /// between cannot miss the queue: it either sees the message in
    /// `after_turn` or the call falls through to a normal start.
    pub fn enqueue(
        &self,
        session_id: &str,
        add: impl FnOnce() -> Result<(), String>,
    ) -> Result<bool, String> {
        let running = self.running.lock().unwrap();
        if running.contains_key(session_id) {
            add()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Starts a turn; refuses while one runs on the session.
    pub fn start(&self, session_id: &str, place: Place, job: Job) -> Result<(), String> {
        self.start_with(session_id, place, job, false)
    }

    /// `start`, with the human away when `autonomous` (`Turn::autonomous`),
    /// for this turn and the child sessions it runs.
    pub fn start_with(
        &self,
        session_id: &str,
        place: Place,
        job: Job,
        autonomous: bool,
    ) -> Result<(), String> {
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
            let mut job = job;
            loop {
                if let Err(e) = run(
                    &settings,
                    &notifier,
                    &session_id,
                    &place.workdir,
                    &job,
                    &stop,
                    autonomous,
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
                let mut running = running.lock().unwrap();
                match next_job(
                    &settings,
                    &notifier,
                    &session_id,
                    &place.workdir,
                    stop.load(Ordering::Relaxed),
                ) {
                    Ok(Some(next)) => {
                        drop(running);
                        job = next;
                    }
                    Ok(None) => {
                        running.remove(&session_id);
                        drop(running);
                        // Whoever waits for the turn to end (the ideas queue) wakes now.
                        notifier.notify();
                        break;
                    }
                    Err(e) => {
                        eprintln!("fennec: session {session_id}: queue: {e}");
                        running.remove(&session_id);
                        drop(running);
                        notifier.notify();
                        break;
                    }
                }
            }
        });
        Ok(())
    }
}

/// Opens the session's store, resolves its model choice and hands the turn
/// (and its tools) to `body`; the shared wiring lives in `turn_build`.
pub(super) fn with_turn<R>(
    settings: &Settings,
    notifier: &Notifier,
    session_id: &str,
    workdir: &Path,
    stop: &AtomicBool,
    autonomous: bool,
    body: impl FnOnce(&mut Turn, &LocalTools) -> Result<R, String>,
) -> Result<R, String> {
    let store = SqliteStore::open(&settings.db_path)?;
    // A workflow's child sessions run on the model the human chose for the session.
    let (chosen, _) = model_routes::session_choice(&store, session_id)?;
    let mut on_event = |_: &Event| {};
    let mut options = turn_build::Options {
        notifier: Some(notifier),
        model_choice: chosen,
        autonomous,
        on_event: &mut on_event,
    };
    turn_build::with_turn(
        settings,
        &store,
        session_id,
        workdir,
        stop,
        &mut options,
        body,
    )
}

fn run(
    settings: &Settings,
    notifier: &Notifier,
    session_id: &str,
    workdir: &Path,
    job: &Job,
    stop: &AtomicBool,
    autonomous: bool,
) -> Result<(), String> {
    with_turn(
        settings,
        notifier,
        session_id,
        workdir,
        stop,
        autonomous,
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
                    autonomous,
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
            Job::Approval {
                call_id,
                choice,
                rule_id,
            } => crate::agent::rule_approval::approve_turn(turn, call_id, *choice, *rule_id)
                .map(|_| ()),
            Job::Waiver { call_id } => {
                crate::agent::waiver::answer_waiver(turn, call_id).map(|_| ())
            }
        },
    )
}

/// The queued message that starts the next job once a turn ends, if any.
/// Called while the session's running lock is held, so a message queued in
/// the meantime is seen here and not lost.
fn next_job(
    settings: &Settings,
    notifier: &Notifier,
    session_id: &str,
    workdir: &Path,
    stopped: bool,
) -> Result<Option<Job>, String> {
    let store = SqliteStore::open(&settings.db_path)?;
    let events = store.events(session_id)?;
    match queue::after_turn(&events, stopped) {
        queue::AfterTurn::Idle | queue::AfterTurn::Hold => Ok(None),
        queue::AfterTurn::Cancel(ids) => {
            let log = NotifyingLog {
                log: &store,
                notifier,
            };
            for id in ids {
                log.append(session_id, &Event::QueueCancelled { queue_id: id })?;
            }
            Ok(None)
        }
        queue::AfterTurn::Start(q) => {
            let log = NotifyingLog {
                log: &store,
                notifier,
            };
            log.append(
                session_id,
                &Event::QueueDrained {
                    queue_id: q.queue_id.clone(),
                },
            )?;
            let input = resolve(&q.content, &skills::discover(workdir));
            Ok(Some(Job::Input(input, Vec::new())))
        }
    }
}
