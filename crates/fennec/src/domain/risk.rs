//! Pure risk-zone model and deterministic project risk measurements.

use serde::Serialize;

pub const CHURN_DAYS: u32 = 90;
pub const HOT_CHURN: usize = 5;

const AI_PROVIDERS: &[&str] = &[
    "claude",
    "anthropic",
    "copilot",
    "cursor",
    "codex",
    "openai",
    "gemini",
    "devin",
    "aider",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authorship {
    Human,
    Integrated,
    Ai,
}

/// What the adapter measured for one source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFacts {
    pub path: String,
    pub lines: usize,
    pub unreviewed_lines: usize,
    pub churn: usize,
    pub tested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Signal {
    Churn,
    Untested,
    Unreviewed,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskZone {
    pub path: String,
    pub signals: Vec<Signal>,
    pub churn: usize,
    pub lines: usize,
    pub unreviewed_percent: u8,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskReport {
    pub files: usize,
    pub unreviewed_percent: u8,
    pub untested: usize,
    pub churning: usize,
    pub unreviewed: usize,
    pub churn_days: u32,
    pub zones: Vec<RiskZone>,
}

pub fn authorship(message: &str) -> Authorship {
    let mut has_ai = false;

    for line in message.lines() {
        if line.starts_with("Fennec-Session:") {
            return Authorship::Integrated;
        }
        if is_ai_coauthor(line) {
            has_ai = true;
        }
    }

    if has_ai {
        Authorship::Ai
    } else {
        Authorship::Human
    }
}

pub fn report(files: Vec<FileFacts>) -> RiskReport {
    let measured = files.len();
    let total_lines: usize = files.iter().map(|file| file.lines).sum();
    let unreviewed_lines: usize = files.iter().map(|file| file.unreviewed_lines).sum();

    let untested = files.iter().filter(|file| !file.tested).count();
    let churning = files.iter().filter(|file| file.churn >= HOT_CHURN).count();
    let unreviewed = files.iter().filter(|file| is_unreviewed_file(file)).count();

    let mut zones: Vec<RiskZone> = files.into_iter().filter_map(zone_for).collect();
    zones.sort_by(|left, right| {
        right
            .signals
            .len()
            .cmp(&left.signals.len())
            .then_with(|| right.churn.cmp(&left.churn))
            .then_with(|| right.unreviewed_percent.cmp(&left.unreviewed_percent))
            .then_with(|| left.path.cmp(&right.path))
    });

    RiskReport {
        files: measured,
        unreviewed_percent: percent_round(unreviewed_lines, total_lines),
        untested,
        churning,
        unreviewed,
        churn_days: CHURN_DAYS,
        zones,
    }
}

pub fn is_tested(path: &str, content: &str, test_paths: &[&str]) -> bool {
    if is_inline_test(path, content) {
        return true;
    }

    let source_stem = file_stem(file_name(path));
    test_paths
        .iter()
        .any(|test_path| test_stem(test_path) == source_stem)
}

fn is_ai_coauthor(line: &str) -> bool {
    let lowered = line.to_ascii_lowercase();
    let Some(value) = lowered.strip_prefix("co-authored-by:") else {
        return false;
    };

    AI_PROVIDERS.iter().any(|provider| value.contains(provider))
}

fn is_unreviewed_file(file: &FileFacts) -> bool {
    file.lines > 0 && file.unreviewed_lines == file.lines
}

fn zone_for(file: FileFacts) -> Option<RiskZone> {
    let unreviewed = is_unreviewed_file(&file);
    let untested = !file.tested;
    let churning = file.churn >= HOT_CHURN;

    if !unreviewed && !untested && !churning {
        return None;
    }

    let mut signals = Vec::new();
    if unreviewed {
        signals.push(Signal::Unreviewed);
    }
    if untested {
        signals.push(Signal::Untested);
    }
    if churning {
        signals.push(Signal::Churn);
    }

    Some(RiskZone {
        path: file.path,
        signals,
        churn: file.churn,
        lines: file.lines,
        unreviewed_percent: percent_round(file.unreviewed_lines, file.lines),
    })
}

fn percent_round(part: usize, total: usize) -> u8 {
    if total == 0 {
        return 0;
    }

    let part = part as u128;
    let total = total as u128;
    let percent = (part * 100 + total / 2) / total;

    percent.min(100) as u8
}

fn is_inline_test(path: &str, content: &str) -> bool {
    let name = file_name(path);

    if name.ends_with(".rs") {
        content.contains("#[cfg(test)]") || content.contains("#[test]")
    } else if name.ends_with(".py") {
        content.contains("\ndef test_")
    } else {
        false
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn file_stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(position) if position > 0 => &name[..position],
        _ => name,
    }
}

fn test_stem(path: &str) -> &str {
    let base = file_stem(file_name(path));

    base.strip_suffix(".test")
        .or_else(|| base.strip_suffix(".spec"))
        .or_else(|| base.strip_suffix("_test"))
        .or_else(|| base.strip_prefix("test_"))
        .unwrap_or(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorship_detects_human() {
        assert_eq!(authorship("Fix bug\n\nNo trailers."), Authorship::Human);
        assert_eq!(
            authorship("Fix bug\n\nCo-Authored-By: Bob <bob@example.com>"),
            Authorship::Human
        );
    }

    #[test]
    fn authorship_detects_each_ai_provider() {
        for provider in AI_PROVIDERS {
            let message = format!("Fix bug\n\nCo-Authored-By: {provider} <bot@example.com>");
            assert_eq!(authorship(&message), Authorship::Ai);
        }
    }

    #[test]
    fn authorship_is_case_insensitive_for_ai_coauthor() {
        let message = "Fix bug\n\nCO-AUTHORED-BY: Claude <bot@example.com>";
        assert_eq!(authorship(message), Authorship::Ai);
    }

    #[test]
    fn integrated_wins_over_ai_coauthor() {
        let message = "Fix bug\n\nFennec-Session: 42\nCo-Authored-By: Claude <bot@example.com>";
        assert_eq!(authorship(message), Authorship::Integrated);
    }

    #[test]
    fn report_counts_percent_and_sorts_zones() {
        let files = vec![
            FileFacts {
                path: "b.rs".into(),
                lines: 4,
                unreviewed_lines: 4,
                churn: 6,
                tested: true,
            },
            FileFacts {
                path: "a.rs".into(),
                lines: 3,
                unreviewed_lines: 2,
                churn: 5,
                tested: false,
            },
            FileFacts {
                path: "c.rs".into(),
                lines: 5,
                unreviewed_lines: 0,
                churn: 7,
                tested: true,
            },
            FileFacts {
                path: "d.rs".into(),
                lines: 1,
                unreviewed_lines: 0,
                churn: 0,
                tested: true,
            },
        ];

        let report = report(files);

        assert_eq!(report.files, 4);
        assert_eq!(report.unreviewed_percent, 46);
        assert_eq!(report.untested, 1);
        assert_eq!(report.churning, 3);
        assert_eq!(report.unreviewed, 1);
        assert_eq!(report.churn_days, CHURN_DAYS);

        let paths: Vec<&str> = report.zones.iter().map(|zone| zone.path.as_str()).collect();
        assert_eq!(paths, vec!["b.rs", "a.rs", "c.rs"]);
    }

    #[test]
    fn zone_signals_use_stable_domain_order() {
        let files = vec![FileFacts {
            path: "hot.rs".into(),
            lines: 1,
            unreviewed_lines: 1,
            churn: HOT_CHURN,
            tested: false,
        }];

        let report = report(files);
        let zone = &report.zones[0];

        assert_eq!(
            zone.signals,
            vec![Signal::Unreviewed, Signal::Untested, Signal::Churn]
        );
    }

    #[test]
    fn percent_round_rounds_to_nearest() {
        assert_eq!(percent_round(0, 0), 0);
        assert_eq!(percent_round(1, 3), 33);
        assert_eq!(percent_round(2, 3), 67);
        assert_eq!(percent_round(1, 8), 13);
        assert_eq!(percent_round(10, 10), 100);
    }

    #[test]
    fn is_tested_detects_inline_rust_tests() {
        assert!(is_tested(
            "src/turn.rs",
            "fn main() {}\n\n#[cfg(test)]\nmod tests {}",
            &[]
        ));
        assert!(is_tested("src/turn.rs", "#[test]\nfn it_works() {}", &[]));
    }

    #[test]
    fn is_tested_detects_inline_python_tests() {
        assert!(is_tested(
            "src/cli.py",
            "import sys\n\ndef test_cli():\n    assert True",
            &[]
        ));
    }

    #[test]
    fn is_tested_detects_sibling_test_file() {
        assert!(is_tested("src/api.ts", "export {}", &["src/api.test.ts"]));
    }

    #[test]
    fn is_tested_detects_underscore_and_prefix_test_names() {
        assert!(is_tested(
            "src/turn.rs",
            "fn turn() {}",
            &["src/turn_test.rs"]
        ));
        assert!(is_tested(
            "src/cli.py",
            "def cli() {}",
            &["tests/test_cli.py"]
        ));
        assert!(is_tested("src/turn.rs", "fn turn() {}", &["tests/turn.rs"]));
    }

    #[test]
    fn is_tested_is_false_without_matching_test() {
        assert!(!is_tested(
            "src/turn.rs",
            "fn turn() {}",
            &["src/other_test.rs"]
        ));
    }
}
