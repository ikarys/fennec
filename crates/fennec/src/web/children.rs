//! Child sessions for `fresh` agent nodes and parallel branches: ordinary
//! sessions with a parent, run on their own turn.

use super::notifier::Notifier;
use super::turns::with_turn;
use crate::agent::ports::{AgentRun, ChildSessions, EventLog};
use crate::agent::turn::run_agent_node;
use crate::config::Settings;
use crate::domain::conversation::last_assistant_text;
use crate::domain::stats::NodeTokens;
use crate::domain::workflow::stats::node_tokens;
use crate::store::SqliteStore;
use std::path::Path;
use std::sync::atomic::AtomicBool;

pub(super) struct Children<'a> {
    pub(super) settings: &'a Settings,
    pub(super) notifier: &'a Notifier,
    pub(super) workdir: &'a Path,
}

impl ChildSessions for Children<'_> {
    fn create(&self, parent_session_id: &str, title: &str) -> Result<String, String> {
        let store = SqliteStore::open(&self.settings.db_path)?;
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
            self.settings,
            self.notifier,
            child_session_id,
            self.workdir,
            stop,
            |turn, _| {
                let before = turn.log.events(child_session_id)?.len();
                let reason = run_agent_node(turn, agent_id, prompt)?;
                let events = turn.log.events(child_session_id)?;
                let text = last_assistant_text(&events[before..]);
                Ok(AgentRun { reason, text })
            },
        )
    }

    fn model_tokens(&self, child_session_id: &str) -> Option<NodeTokens> {
        let store = SqliteStore::open(&self.settings.db_path).ok()?;
        let events = store.events(child_session_id).ok()?;
        node_tokens(&events)
    }
}
