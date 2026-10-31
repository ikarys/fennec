//! A fennec-free backlog of ideas.
//!
//! An idea is jotted, framed into a plan, and run as a queue. The source of
//! truth is an append-only log of [`BacklogEvent`]s; [`fold`] rebuilds the
//! current [`Backlog`] from that log.
//!
//! This crate has no I/O, no database, and no fennec types, so another
//! product can reuse the same backlog logic.

pub mod attachment;
pub mod event;
pub mod fold;
pub mod queue;
pub mod status;
pub mod summary;
pub mod title;

pub use attachment::IdeaAttachment;
pub use event::BacklogEvent;
pub use fold::{Backlog, Idea, IdeaState, QueueRun, fold, idea_for_session};
pub use queue::{CapHit, Caps, cap_reached, next_ready, queue_order};
pub use status::{Action, MenuAction, Progress, STAGES, Stage, Status, status};
pub use summary::{RunSummary, summarize};
pub use title::title;
