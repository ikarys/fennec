//! Turns started from the UI: each runs on its own thread, reports through
//! the event log, and can be stopped. One turn at a time per session.

use super::children::Children;
use super::model_routes;
use crate::agent::ports::EventLog;
use crate::agent::turn::{Turn, compact_session, replay_turn, retry_turn, run_turn};
use crate::agent::workflow::{WorkflowEnv, WorkflowJob, run_workflow};
use crate::config::Settings;
use crate::domain::attachment::Attachment;
use crate::domain::event::{DoneReason, Event};
use crate::domain::queue;
use crate::domain::rule_approval::Choice;
use crate::domain::slash::{TurnInput, resolve};
use crate::domain::workflow::run as run_state;
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
        let ctx = TurnContext {
            settings: self.settings.clone(),
            notifier: self.notifier.clone(),
            running: self.running.clone(),
            session_id: session_id.to_string(),
            place,
            stop,
            autonomous,
        };
        std::thread::spawn(move || run_turn_loop(ctx, job));
        Ok(())
    }
}

/// Everything a turn thread needs to run one session's turns: the shared
/// wiring and where the session runs.
struct TurnContext {
    settings: Settings,
    notifier: Arc<Notifier>,
    running: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    session_id: String,
    place: Place,
    stop: Arc<AtomicBool>,
    autonomous: bool,
}

/// The turn's loop, one thread per session: runs the job, then whatever the
/// queue drops in, until nothing is left. A child session that just finished
/// re-drives its parent's workflow (`resume_blocked_parent`).
fn run_turn_loop(ctx: TurnContext, job: Job) {
    let mut job = job;
    loop {
        // A panic in the turn (a tool, a parser) must not leave the
        // session running forever: its turn ends as cut, the loop goes on.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run(
                &ctx.settings,
                &ctx.notifier,
                &ctx.session_id,
                &ctx.place.workdir,
                &job,
                &ctx.stop,
                ctx.autonomous,
            )
        }));
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(e)) => eprintln!("fennec: session {}: {e}", ctx.session_id),
            Err(_) => {
                eprintln!("fennec: session {}: the turn panicked", ctx.session_id);
                let ended = SqliteStore::open(&ctx.settings.db_path)
                    .and_then(|store| store.end_cut_turn(&ctx.session_id));
                if let Err(e) = ended {
                    eprintln!(
                        "fennec: session {}: ending the cut turn: {e}",
                        ctx.session_id
                    );
                }
                ctx.notifier.notify();
            }
        }
        // Whatever the job did, what the agent wrote is recorded as
        // fennec's commits for the human to cherry-pick.
        if let Some(wt) = &ctx.place.worktree {
            let message = format!("fennec: agent work in session {}", ctx.session_id);
            if let Err(e) = worktree::commit(wt, &message) {
                eprintln!("fennec: session {}: commit: {e}", ctx.session_id);
            }
        }
        let mut guard = ctx.running.lock().unwrap();
        match next_job(
            &ctx.settings,
            &ctx.notifier,
            &ctx.session_id,
            &ctx.place.workdir,
            ctx.stop.load(Ordering::Relaxed),
        ) {
            Ok(Some(next)) => {
                drop(guard);
                job = next;
            }
            Ok(None) => {
                guard.remove(&ctx.session_id);
                drop(guard);
                resume_blocked_parent(&ctx);
                // Whoever waits for the turn to end (the ideas queue) wakes now.
                ctx.notifier.notify();
                break;
            }
            Err(e) => {
                eprintln!("fennec: session {}: queue: {e}", ctx.session_id);
                guard.remove(&ctx.session_id);
                drop(guard);
                ctx.notifier.notify();
                break;
            }
        }
    }
}

/// A child session that just finished re-drives its parent's workflow: the
/// node it ran in can now finish on its output. The parent's run must be
/// active with that node open (a child that asked a question left it open); a
/// child that is still waiting does not re-drive it.
fn resume_blocked_parent(ctx: &TurnContext) {
    let store = match SqliteStore::open(&ctx.settings.db_path) {
        Ok(store) => store,
        Err(_) => return,
    };
    let Some(child) = store.session(&ctx.session_id).ok().flatten() else {
        return;
    };
    let Some(parent_id) = child.parent_id else {
        return;
    };
    let child_events = match store.events(&ctx.session_id) {
        Ok(events) => events,
        Err(_) => return,
    };
    // Only a child whose last turn completed re-drives the parent; one that
    // is still waiting (answered and asked again) does not.
    let completed = child_events
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::ChatDone { reason, .. } => Some(*reason),
            _ => None,
        })
        .is_some_and(|reason| reason == DoneReason::Complete);
    if !completed {
        return;
    }
    let parent_events = match store.events(&parent_id) {
        Ok(events) => events,
        Err(_) => return,
    };
    let Some(parent_run) = run_state::current_run(&parent_events) else {
        return;
    };
    // The nodes this run started in this child: the one it is blocked on is the
    // open node among them.
    let child_nodes: HashSet<&str> = parent_events
        .iter()
        .filter_map(|event| match event {
            Event::WorkflowNodeStarted {
                run_id,
                node_id,
                child_session_id: Some(child),
                ..
            } if run_id == &parent_run.run_id && child == &ctx.session_id => Some(node_id.as_str()),
            _ => None,
        })
        .collect();
    let blocked_on_child = parent_run
        .open
        .iter()
        .any(|open| child_nodes.contains(open.node_id.as_str()));
    if !blocked_on_child {
        return;
    }
    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut guard = ctx.running.lock().unwrap();
        if guard.contains_key(&parent_id) {
            return;
        }
        guard.insert(parent_id.clone(), stop.clone());
    }
    // The parent shares the child's workdir and root worktree, so the child's
    // place is the parent's.
    let parent_ctx = TurnContext {
        settings: ctx.settings.clone(),
        notifier: ctx.notifier.clone(),
        running: ctx.running.clone(),
        session_id: parent_id,
        place: ctx.place.clone(),
        stop,
        autonomous: ctx.autonomous,
    };
    std::thread::spawn(move || run_turn_loop(parent_ctx, Job::Workflow(WorkflowJob::Resume)));
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
            Job::Retry => retry_turn(turn).map(|_| ()),
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
