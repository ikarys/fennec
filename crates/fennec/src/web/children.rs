//! Child sessions for `fresh` agent nodes and parallel branches: ordinary
//! sessions with a parent, run on their own turn.

use crate::agent::ports::{AgentRun, ChildSessions, EventLog};
use crate::agent::turn::run_child_agent_node;
use crate::config::Settings;
use crate::domain::conversation::{last_done, node_output};
use crate::domain::event::DoneReason;
use crate::domain::workflow::run;
use crate::notifier::Notifier;
use crate::store::SqliteStore;
use crate::web::turns::with_turn;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

#[derive(Clone, Copy)]
pub(super) struct Children<'a> {
    pub(super) settings: &'a Settings,
    pub(super) notifier: &'a Notifier,
    pub(super) workdir: &'a Path,
    /// The parent's turn runs with the human away: so do its children.
    pub(super) autonomous: bool,
    /// The turn is a `delegate` call's child: what it leaves for the human goes in its answer.
    pub(super) delegated: bool,
    /// The delegations this turn started, counted against the Settings cap; None for a
    /// workflow's children, which have no cap.
    pub(super) delegations: Option<&'a AtomicU32>,
}

impl ChildSessions for Children<'_> {
    fn create(&self, parent_session_id: &str, title: &str) -> Result<String, String> {
        let store = SqliteStore::open(&self.settings.db_path)?;
        // Count only the delegations a turn starts against the human's cap; workflow
        // children have no counter.
        if let Some(count) = self.delegations {
            let max = store.preferences()?.delegation.max_per_turn;
            if max > 0 && count.load(Ordering::Relaxed) >= max {
                return Err(format!(
                    "The human allows at most {max} delegations per turn (Settings). Do the rest of the task yourself."
                ));
            }
            count.fetch_add(1, Ordering::Relaxed);
        }
        Ok(store.create_child_session(parent_session_id, title)?)
    }

    fn send(
        &self,
        child_session_id: &str,
        agent_id: &str,
        prompt: &str,
        stop: &AtomicBool,
    ) -> Result<AgentRun, String> {
        // Each call opens its own store connection: SQLite connections are
        // per thread.
        with_turn(
            self,
            child_session_id,
            stop,
            // A child session never delegates: one level, no recursion.
            None,
            |turn, _| {
                let before = turn.log.events(child_session_id)?.len();
                let reason = run_child_agent_node(turn, agent_id, prompt)?;
                let events = turn.log.events(child_session_id)?;
                let text = node_output(&events[before..]);
                Ok(AgentRun { reason, text })
            },
        )
    }

    fn last_run(&self, child_session_id: &str) -> Result<AgentRun, String> {
        let store = SqliteStore::open(&self.settings.db_path)?;
        let events = store.events(child_session_id)?;
        let reason = last_done(&events).ok_or("the child session has no turn")?;
        let text = node_output(&events);
        Ok(AgentRun { reason, text })
    }
}

/// The parent session whose run resumes now that `child_session_id`'s turn
/// ended: the child completed (the human answered its question) and the
/// parent's run waits on it. `None` otherwise.
pub(super) fn parent_to_resume(
    store: &SqliteStore,
    child_session_id: &str,
) -> Result<Option<String>, String> {
    if last_done(&store.events(child_session_id)?) != Some(DoneReason::Complete) {
        return Ok(None);
    }
    let Some(parent) = store.session(child_session_id)?.and_then(|s| s.parent_id) else {
        return Ok(None);
    };
    Ok(run::waits_on_child(&store.events(&parent)?, child_session_id).then_some(parent))
}
