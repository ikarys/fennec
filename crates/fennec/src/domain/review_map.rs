//! The order in which the human reviews a branch's files, from
//! deterministic signals.

use std::cmp::Reverse;

use serde::Serialize;

use super::integration::DiffChecks;
use super::test_files::is_test_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewSignal {
    Protected,
    Sensitive,
    Bypass,
    Architecture,
    ConflictMarker,
    TestsWeakened,
    Hot,
    Untested,
    Large,
}

pub struct FileFacts {
    /// Project-relative path.
    pub path: String,
    pub added: u64,
    pub removed: u64,
    pub protected: bool,
    pub hot: bool,
    pub tested: bool,
}

pub const LARGE_CHANGE: u64 = 200; // added + removed

/// The deterministic signals for one file, in enum order and without
/// duplicates.
pub fn signals(file: &FileFacts, checks: &DiffChecks) -> Vec<ReviewSignal> {
    let mut out = Vec::new();

    if file.protected {
        out.push(ReviewSignal::Protected);
    }
    if checks.sensitive.contains(&file.path) {
        out.push(ReviewSignal::Sensitive);
    }
    if checks
        .suppressions
        .iter()
        .any(|suppression| suppression.path == file.path)
    {
        out.push(ReviewSignal::Bypass);
    }
    if checks
        .violations
        .iter()
        .any(|violation| violation.path == file.path)
    {
        out.push(ReviewSignal::Architecture);
    }
    if checks.markers.contains(&file.path) {
        out.push(ReviewSignal::ConflictMarker);
    }
    if checks
        .weakened
        .as_ref()
        .is_some_and(|weakened| weakened.paths.contains(&file.path))
    {
        out.push(ReviewSignal::TestsWeakened);
    }
    if file.hot {
        out.push(ReviewSignal::Hot);
    }
    if !file.tested && !is_test_path(&file.path) {
        out.push(ReviewSignal::Untested);
    }
    if file.added + file.removed >= LARGE_CHANGE {
        out.push(ReviewSignal::Large);
    }

    out
}

/// One entry per file, riskiest first.
pub fn order(files: &[FileFacts], checks: &DiffChecks) -> Vec<(String, Vec<ReviewSignal>)> {
    let mut entries: Vec<(&FileFacts, Vec<ReviewSignal>)> = files
        .iter()
        .map(|file| (file, signals(file, checks)))
        .collect();
    entries.sort_by_key(|(file, signals)| {
        let gate = signals.iter().filter(|signal| is_gate(**signal)).count();
        (
            Reverse(gate),
            Reverse(signals.len()),
            Reverse(file.added + file.removed),
            file.path.clone(),
        )
    });
    entries
        .into_iter()
        .map(|(file, signals)| (file.path.clone(), signals))
        .collect()
}

/// The signals every file of the map carries: said once above the list,
/// they differentiate nothing on the rows. Empty under two files.
pub fn common(entries: &[(String, Vec<ReviewSignal>)]) -> Vec<ReviewSignal> {
    if entries.len() < 2 {
        return Vec::new();
    }
    let first = &entries[0].1;
    first
        .iter()
        .copied()
        .filter(|signal| {
            entries[1..]
                .iter()
                .all(|(_, signals)| signals.contains(signal))
        })
        .collect()
}

/// Gate signals are the ones that are also integration blockers.
fn is_gate(signal: ReviewSignal) -> bool {
    matches!(
        signal,
        ReviewSignal::Protected
            | ReviewSignal::Sensitive
            | ReviewSignal::Bypass
            | ReviewSignal::Architecture
            | ReviewSignal::ConflictMarker
            | ReviewSignal::TestsWeakened
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::architecture::Violation;
    use crate::domain::bypass::{Suppression, TestsWeakened};
    use crate::domain::integration::DiffChecks;

    fn file(
        path: &str,
        added: u64,
        removed: u64,
        protected: bool,
        hot: bool,
        tested: bool,
    ) -> FileFacts {
        FileFacts {
            path: path.into(),
            added,
            removed,
            protected,
            hot,
            tested,
        }
    }

    #[test]
    fn signals_reports_each_kind() {
        let protected = file("src/a.rs", 0, 0, true, false, true);
        assert_eq!(
            signals(&protected, &DiffChecks::default()),
            vec![ReviewSignal::Protected]
        );

        let bypass_checks = DiffChecks {
            suppressions: vec![Suppression {
                path: "src/a.rs".into(),
                line: "1".into(),
                marker: "fennec:ignore".into(),
            }],
            ..Default::default()
        };
        let bypass = file("src/a.rs", 0, 0, false, false, true);
        assert_eq!(signals(&bypass, &bypass_checks), vec![ReviewSignal::Bypass]);

        let architecture_checks = DiffChecks {
            violations: vec![Violation {
                path: "src/a.rs".into(),
                line: "1".into(),
                prefix: "crate::".into(),
                forbidden: "std".into(),
            }],
            ..Default::default()
        };
        let architecture = file("src/a.rs", 0, 0, false, false, true);
        assert_eq!(
            signals(&architecture, &architecture_checks),
            vec![ReviewSignal::Architecture]
        );

        let marker_checks = DiffChecks {
            markers: vec!["src/a.rs".into()],
            ..Default::default()
        };
        let marker = file("src/a.rs", 0, 0, false, false, true);
        assert_eq!(
            signals(&marker, &marker_checks),
            vec![ReviewSignal::ConflictMarker]
        );

        let weakened_checks = DiffChecks {
            weakened: Some(TestsWeakened {
                tests_removed: 0,
                assertions_removed: 0,
                paths: vec!["src/a.rs".into()],
            }),
            ..Default::default()
        };
        let weakened = file("src/a.rs", 0, 0, false, false, true);
        assert_eq!(
            signals(&weakened, &weakened_checks),
            vec![ReviewSignal::TestsWeakened]
        );

        let hot = file("src/a.rs", 0, 0, false, true, true);
        assert_eq!(
            signals(&hot, &DiffChecks::default()),
            vec![ReviewSignal::Hot]
        );

        let untested = file("src/a.rs", 0, 0, false, false, false);
        assert_eq!(
            signals(&untested, &DiffChecks::default()),
            vec![ReviewSignal::Untested]
        );

        let large = file("src/a.rs", 200, 0, false, false, true);
        assert_eq!(
            signals(&large, &DiffChecks::default()),
            vec![ReviewSignal::Large]
        );
    }

    #[test]
    fn test_file_is_never_untested() {
        let path = "tests/a.rs";
        assert!(is_test_path(path));

        let file = FileFacts {
            path: path.into(),
            added: 0,
            removed: 0,
            protected: false,
            hot: false,
            tested: false,
        };

        let signals = signals(&file, &DiffChecks::default());

        assert!(!signals.contains(&ReviewSignal::Untested));
    }

    #[test]
    fn order_puts_gate_before_count_before_size_before_path() {
        let gate = file("gate.rs", 0, 0, true, false, true);
        let count = file("count.rs", 0, 0, false, true, false);
        let gate_first = order(&[count, gate], &DiffChecks::default());
        assert_eq!(gate_first[0].0, "gate.rs");

        let two_signals = file("two.rs", 10, 0, false, true, false);
        let one_large = file("one.rs", 200, 0, false, false, true);
        let count_before_size = order(&[one_large, two_signals], &DiffChecks::default());
        assert_eq!(count_before_size[0].0, "two.rs");

        let big = file("big.rs", 300, 0, false, false, true);
        let small = file("small.rs", 200, 0, false, false, true);
        let size_before_path = order(&[small, big], &DiffChecks::default());
        assert_eq!(size_before_path[0].0, "big.rs");

        let b = file("b.rs", 0, 0, false, false, true);
        let a = file("a.rs", 0, 0, false, false, true);
        let path_last = order(&[b, a], &DiffChecks::default());
        assert_eq!(path_last[0].0, "a.rs");
    }

    #[test]
    fn sensitive_file_ranks_before_plain_file_and_carries_signal() {
        let checks = DiffChecks {
            sensitive: vec!["src/auth/login.rs".into()],
            ..Default::default()
        };
        let sensitive = file("src/auth/login.rs", 10, 0, false, false, true);
        let plain = file("src/plain.rs", 10, 0, false, false, true);

        let ordered = order(&[plain, sensitive], &checks);

        assert_eq!(ordered[0].0, "src/auth/login.rs");
        assert_eq!(ordered[0].1, vec![ReviewSignal::Sensitive]);
        assert_eq!(ordered[1].1, vec![]);
    }

    #[test]
    fn common_keeps_only_the_signals_every_file_carries() {
        let checks = DiffChecks::default();
        let large = file("large.rs", 200, 0, false, true, false);
        let plain = file("plain.rs", 0, 0, false, true, false);
        let entries = order(
            &[file("plain.rs", 0, 0, false, true, false), large],
            &checks,
        );
        assert_eq!(
            common(&entries),
            vec![ReviewSignal::Hot, ReviewSignal::Untested]
        );

        let single = order(&[plain], &checks);
        assert_eq!(common(&single), Vec::<ReviewSignal>::new());
    }
}
