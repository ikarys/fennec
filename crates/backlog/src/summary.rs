use crate::status::Status;
use serde::Serialize;

/// Counts for a queue run's current statuses.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub started: usize,
    pub running: usize,
    pub review: usize,
    pub question: usize,
    pub halted: usize,
    pub failed: usize,
    pub integrated: usize,
}

/// Count statuses for a run summary.
///
/// `started` is the total number of statuses. Inbox, framing, and queued ideas
/// count only in `started` because they are not active run outcomes.
pub fn summarize(statuses: &[Status]) -> RunSummary {
    let mut summary = RunSummary::default();

    for status in statuses {
        summary.started += 1;

        match status {
            Status::Running => summary.running += 1,
            Status::Review => summary.review += 1,
            Status::Question => summary.question += 1,
            Status::Halted => summary.halted += 1,
            Status::Failed => summary.failed += 1,
            // A human-completed idea is a done run outcome, like an integrated one.
            Status::Integrated | Status::Completed => summary.integrated += 1,
            Status::Inbox | Status::Framing | Status::Queued => {}
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_counts_active_statuses_and_started_total() {
        let statuses = vec![
            Status::Running,
            Status::Review,
            Status::Question,
            Status::Halted,
            Status::Failed,
            Status::Integrated,
            Status::Completed,
            Status::Inbox,
            Status::Framing,
            Status::Queued,
        ];

        let summary = summarize(&statuses);

        assert_eq!(summary.started, 10);
        assert_eq!(summary.running, 1);
        assert_eq!(summary.review, 1);
        assert_eq!(summary.question, 1);
        assert_eq!(summary.halted, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.integrated, 2);
    }
}
