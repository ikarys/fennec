//! Turns started from the UI: each runs on its own thread, reports through
//! the event log, and can be stopped. One turn at a time per session.

use crate::agent::ports::{ChildSessions, EventLog};
use crate::agent::turn::{Turn, compact_session, replay_turn, retry_turn, run_turn};
use crate::agent::workflow::{WorkflowEnv, WorkflowJob, answer_in_node, run_workflow};
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
use crate::web::children::Children;
use crate::web::routes::model_routes;
use crate::worktree::{self, Worktree};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// Where a session's turns run: its worktree when it has one, the project's
/// directory otherwise.
#[derive(Clone)]
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
    /// The human retries a turn that ended in an error.
    Retry,
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

#[derive(Clone)]
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
        crate::locks::lock(&self.running).contains_key(session_id)
    }

    /// A copy of the running map's keys, for the server-wide notice stream.
    pub fn running_ids(&self) -> HashSet<String> {
        crate::locks::lock(&self.running).keys().cloned().collect()
    }

    pub fn stop(&self, session_id: &str) {
        if let Some(flag) = crate::locks::lock(&self.running).get(session_id) {
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
        let running = crate::locks::lock(&self.running);
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
            let mut running = crate::locks::lock(&self.running);
            if running.contains_key(session_id) {
                return Err("a turn is already running".into());
            }
            running.insert(session_id.to_string(), stop.clone());
        }
        let settings = self.settings.clone();
        let notifier = self.notifier.clone();
        let running = self.running.clone();
        let session_id = session_id.to_string();
        let turns = self.clone();
        std::thread::spawn(move || {
            let mut job = job;
            loop {
                // A panic in the turn (a tool, a parser) must not leave the
                // session running forever: its turn ends as cut, the loop goes on.
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(
                        &settings,
                        &notifier,
                        &session_id,
                        &place.workdir,
                        &job,
                        &stop,
                        autonomous,
                    )
                }));
                match outcome {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => eprintln!("fennec: session {session_id}: {e}"),
                    Err(_) => {
                        eprintln!("fennec: session {session_id}: the turn panicked");
                        let ended = SqliteStore::open(&settings.db_path)
                            .and_then(|store| store.end_cut_turn(&session_id));
                        if let Err(e) = ended {
                            eprintln!("fennec: session {session_id}: ending the cut turn: {e}");
                        }
                        notifier.notify();
                    }
                }
                // Whatever the job did, what the agent wrote is recorded as
                // fennec's commits for the human to cherry-pick.
                if let Some(wt) = &place.worktree {
                    let message = format!("fennec: agent work in session {session_id}");
                    if let Err(e) = worktree::commit(wt, &message) {
                        eprintln!("fennec: session {session_id}: commit: {e}");
                    }
                }
                let mut running = crate::locks::lock(&running);
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
                        resume_parent(&turns, &session_id, &place, autonomous);
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

/// A workflow step's child session completed after the human answered its
/// question: the parent's run resumes on its own, in the same place (a child
/// shares its root session's place).
fn resume_parent(turns: &Turns, child_session_id: &str, place: &Place, autonomous: bool) {
    let store = match SqliteStore::open(&turns.settings.db_path) {
        Ok(store) => store,
        Err(e) => {
            eprintln!("fennec: session {child_session_id}: resuming its parent: {e}");
            return;
        }
    };
    match super::children::parent_to_resume(&store, child_session_id) {
        Ok(Some(parent)) => {
            let job = Job::Workflow(WorkflowJob::Resume);
            if let Err(e) = turns.start_with(&parent, place.clone(), job, autonomous) {
                eprintln!("fennec: session {child_session_id}: resuming {parent}: {e}");
            }
        }
        Ok(None) => {}
        Err(e) => eprintln!("fennec: session {child_session_id}: resuming its parent: {e}"),
    }
}

/// Opens the session's store, resolves its model choice and hands the turn
/// (and its tools) to `body`; the shared wiring lives in `turn_build`. `at`
/// is where the turn runs: its settings, notifier, workdir and autonomy;
/// `delegates` where its `delegate` calls run, None for no delegation.
pub(super) fn with_turn<R>(
    at: &Children,
    session_id: &str,
    stop: &AtomicBool,
    delegates: Option<&dyn ChildSessions>,
    body: impl FnOnce(&mut Turn, &LocalTools) -> Result<R, String>,
) -> Result<R, String> {
    let Children {
        settings,
        notifier,
        workdir,
        autonomous,
        delegated,
        delegations: _,
    } = *at;
    let store = SqliteStore::open(&settings.db_path)?;
    // A workflow's child sessions run on the model the human chose for the session.
    let (chosen, _) = model_routes::session_choice(&store, session_id)?;
    let (effort, _) = model_routes::session_effort(settings, &store, session_id)?;
    let mut on_event = |_: &Event| {};
    let mut options = turn_build::Options {
        notifier: Some(notifier),
        model_choice: chosen,
        effort_choice: effort,
        autonomous,
        delegated,
        on_event: &mut on_event,
        children: delegates,
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
    let delegations = AtomicU32::new(0);
    let here = Children {
        settings,
        notifier,
        workdir,
        autonomous,
        delegated: false,
        delegations: None,
    };
    // A delegated agent runs with the human away: nobody answers its
    // questions or approvals, so it decides or is refused, never pauses.
    let delegates = Children {
        autonomous: true,
        delegated: true,
        delegations: Some(&delegations),
        ..here
    };
    with_turn(
        &here,
        session_id,
        stop,
        Some(&delegates),
        |turn, tools| match job {
            Job::Input(TurnInput::Prompt(prompt), attachments) => {
                run_turn(turn, prompt, attachments).map(|_| ())
            }
            Job::Input(TurnInput::Compact, _) => compact_session(turn).map(|_| ()),
            Job::Replay(message_id) => replay_turn(turn, message_id).map(|_| ()),
            Job::Retry => retry_turn(turn).map(|_| ()),
            Job::Workflow(workflow_job) => run_workflow(
                turn,
                &WorkflowEnv {
                    children: &here,
                    commands: tools,
                },
                workflow_job.clone(),
            )
            .map(|_| ()),
            Job::Approval {
                call_id,
                choice,
                rule_id,
            } => answer_in_node(
                turn,
                &WorkflowEnv {
                    children: &here,
                    commands: tools,
                },
                |turn| {
                    crate::agent::rule_approval::answer_approval(turn, call_id, *choice, *rule_id)
                },
            )
            .map(|_| ()),
            Job::Waiver { call_id } => answer_in_node(
                turn,
                &WorkflowEnv {
                    children: &here,
                    commands: tools,
                },
                |turn| crate::agent::waiver::answer_waiver(turn, call_id),
            )
            .map(|_| ()),
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
