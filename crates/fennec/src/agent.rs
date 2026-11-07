//! The application layer: the agent loop, written against the ports in
//! `ports` so that storage, model and tools stay replaceable.

mod ask_user;
mod compaction;
mod delegate;
mod delta_buffer;
pub mod ports;
mod preflight;
mod prompt;
mod propose_criteria;
mod propose_plan;
mod propose_task;
mod queue;
mod record_decision;
mod request_waiver;
mod round;
pub mod rule_approval;
mod rule_check;
mod submit_report;
mod suggest_agent;
#[cfg(test)]
mod test_support;
mod tool_args;
mod tool_calls;
pub mod turn;
pub mod waiver;
pub mod workflow;

#[cfg(test)]
mod tests {
    use crate::domain::agent::KNOWN_TOOLS;

    #[test]
    fn every_loop_tool_is_a_tool_an_agent_may_name() {
        // A name out of KNOWN_TOOLS would let a custom agent ask for a dead tool.
        assert!(KNOWN_TOOLS.contains(&"read_file"));
        assert!(KNOWN_TOOLS.contains(&super::propose_task::NAME));
        assert!(KNOWN_TOOLS.contains(&super::suggest_agent::NAME));
        assert!(KNOWN_TOOLS.contains(&super::delegate::NAME));
        assert!(KNOWN_TOOLS.contains(&super::ask_user::NAME));
        assert!(KNOWN_TOOLS.contains(&super::record_decision::NAME));
        assert!(KNOWN_TOOLS.contains(&super::propose_criteria::NAME));
        assert!(KNOWN_TOOLS.contains(&super::propose_plan::NAME));
        assert!(KNOWN_TOOLS.contains(&super::submit_report::NAME));
        assert!(KNOWN_TOOLS.contains(&super::request_waiver::NAME));
        assert!(!KNOWN_TOOLS.contains(&"rm_rf"));
    }
}
