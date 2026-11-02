//! Append-only SQLite ledger for LLM calls.
//!
//! It stores generic call records (tokens, speed, savings) and answers
//! aggregate queries. It is not a pricing engine, network client, UI, or
//! LLM runtime.

mod call;
pub mod display;
mod ledger;
mod query;

pub use call::{Call, Usage};
pub use ledger::{Error, Ledger};
pub use query::{
    CallRecord, CallRow, Filter, GroupBy, LatestCall, Row, Summary, group_rows, summary_from_rows,
};

#[cfg(test)]
mod tests {
    #[test]
    fn public_api_is_reexported() {
        let ledger = crate::Ledger::open_in_memory().unwrap();
        let summary = ledger.summary(&crate::Filter::default()).unwrap();
        assert_eq!(summary.calls, 0);
    }
}
