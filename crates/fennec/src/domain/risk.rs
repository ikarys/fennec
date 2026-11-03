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
pub struct RiskModule {
    pub path: String,
    pub unreviewed: usize,
    pub untested: usize,
    pub churning: usize,
    pub zones: Vec<RiskZone>,
    pub remedy_prompt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct History {
    pub commits: usize,
    pub ai_commits: usize,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskReport {
    pub files: usize,
    pub commits: usize,
    pub ai_commits: usize,
    pub unreviewed_percent: u8,
    pub untested: usize,
    pub churning: usize,
    pub unreviewed: usize,
    pub churn_days: u32,
    pub modules: Vec<RiskModule>,
    pub remedy_prompt: String,
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

pub fn report(files: Vec<FileFacts>, history: History) -> RiskReport {
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

    let mut modules: Vec<RiskModule> = Vec::new();
    for zone in zones {
        let path = parent_directory(&zone.path);
        let unreviewed = usize::from(zone.signals.contains(&Signal::Unreviewed));
        let untested = usize::from(zone.signals.contains(&Signal::Untested));
        let churning = usize::from(zone.signals.contains(&Signal::Churn));

        if let Some(module) = modules.iter_mut().find(|module| module.path == path) {
            module.unreviewed += unreviewed;
            module.untested += untested;
            module.churning += churning;
            module.zones.push(zone);
        } else {
            modules.push(RiskModule {
                path,
                unreviewed,
                untested,
                churning,
                zones: vec![zone],
                remedy_prompt: String::new(),
            });
        }
    }

    modules.sort_by(|left, right| {
        total_signal_count(right)
            .cmp(&total_signal_count(left))
            .then_with(|| left.path.cmp(&right.path))
    });

    // Each module carries its own brief so the UI can remediate it in isolation.
    let mut scoped_modules: Vec<RiskModule> = Vec::with_capacity(modules.len());
    for mut module in modules {
        module.remedy_prompt = remedy_prompt(std::slice::from_ref(&module));
        scoped_modules.push(module);
    }

    let remedy_prompt = remedy_prompt(&scoped_modules);
    RiskReport {
        files: measured,
        commits: history.commits,
        ai_commits: history.ai_commits,
        unreviewed_percent: percent_round(unreviewed_lines, total_lines),
        untested,
        churning,
        unreviewed,
        churn_days: CHURN_DAYS,
        modules: scoped_modules,
        remedy_prompt,
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

/// The brief the "Remédier" button sends to the `remedy` agent: the zones by module, riskiest first.
fn remedy_prompt(modules: &[RiskModule]) -> String {
    if modules.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "Remediate the risk zones of this project, measured from git, riskiest module first. Signals: unreviewed = more than half of its lines come from AI commits never integrated through fennec; untested = no inline test and no test file of the same stem; churn = touched by {HOT_CHURN} commits or more in the last {CHURN_DAYS} days."
    ));
    lines.push(String::new());

    for (index, module) in modules.iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
        }
        lines.push(format!("## {}", module.path));
        for zone in &module.zones {
            lines.push(format!("- {}: {}", zone.path, zone_signals(zone)));
        }
    }

    format!("{}\n", lines.join("\n"))
}

fn is_ai_coauthor(line: &str) -> bool {
    let lowered = line.to_ascii_lowercase();
    let Some(value) = lowered.strip_prefix("co-authored-by:") else {
        return false;
    };

    AI_PROVIDERS.iter().any(|provider| value.contains(provider))
}

fn is_unreviewed_file(file: &FileFacts) -> bool {
    file.lines > 0 && file.unreviewed_lines * 2 > file.lines
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

fn parent_directory(path: &str) -> String {
    path.rsplit_once('/')
        .map_or(".", |(dir, _)| dir)
        .to_string()
}

fn total_signal_count(module: &RiskModule) -> usize {
    module.zones.iter().map(|zone| zone.signals.len()).sum()
}

fn zone_signals(zone: &RiskZone) -> String {
    zone.signals
        .iter()
        .map(|signal| match signal {
            Signal::Unreviewed => "unreviewed".to_string(),
            Signal::Untested => "untested".to_string(),
            Signal::Churn => format!("churn ({} commits)", zone.churn),
        })
        .collect::<Vec<_>>()
        .join(", ")
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

        let report = report(files, History::default());

        assert_eq!(report.files, 4);
        assert_eq!(report.commits, 0);
        assert_eq!(report.ai_commits, 0);
        assert_eq!(report.unreviewed_percent, 46);
        assert_eq!(report.untested, 1);
        assert_eq!(report.churning, 3);
        assert_eq!(report.unreviewed, 2);
        assert_eq!(report.churn_days, CHURN_DAYS);

        assert_eq!(report.modules.len(), 1);
        let module = &report.modules[0];
        assert_eq!(module.path, ".");
        assert_eq!(module.unreviewed, 2);
        assert_eq!(module.untested, 1);
        assert_eq!(module.churning, 3);

        let paths: Vec<&str> = module.zones.iter().map(|zone| zone.path.as_str()).collect();
        assert_eq!(paths, vec!["a.rs", "b.rs", "c.rs"]);
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

        let report = report(files, History::default());
        let zone = &report.modules[0].zones[0];

        assert_eq!(
            zone.signals,
            vec![Signal::Unreviewed, Signal::Untested, Signal::Churn]
        );
    }

    #[test]
    fn report_groups_zones_by_parent_directory() {
        let files = vec![
            FileFacts {
                path: "src/alpha/two.rs".into(),
                lines: 1,
                unreviewed_lines: 0,
                churn: 0,
                tested: false,
            },
            FileFacts {
                path: "src/alpha/one.rs".into(),
                lines: 2,
                unreviewed_lines: 2,
                churn: 6,
                tested: false,
            },
            FileFacts {
                path: "zeta/one.rs".into(),
                lines: 1,
                unreviewed_lines: 1,
                churn: 6,
                tested: true,
            },
            FileFacts {
                path: "root.rs".into(),
                lines: 1,
                unreviewed_lines: 0,
                churn: 0,
                tested: false,
            },
        ];

        let report = report(files, History::default());

        let module_paths: Vec<&str> = report
            .modules
            .iter()
            .map(|module| module.path.as_str())
            .collect();
        assert_eq!(module_paths, vec!["src/alpha", "zeta", "."]);

        assert_eq!(report.modules[0].unreviewed, 1);
        assert_eq!(report.modules[0].untested, 2);
        assert_eq!(report.modules[0].churning, 1);
        assert_eq!(report.modules[1].unreviewed, 1);
        assert_eq!(report.modules[1].untested, 0);
        assert_eq!(report.modules[1].churning, 1);
        assert_eq!(report.modules[2].unreviewed, 0);
        assert_eq!(report.modules[2].untested, 1);
        assert_eq!(report.modules[2].churning, 0);

        let alpha_zones: Vec<&str> = report.modules[0]
            .zones
            .iter()
            .map(|zone| zone.path.as_str())
            .collect();
        assert_eq!(alpha_zones, vec!["src/alpha/one.rs", "src/alpha/two.rs"]);
        assert_eq!(report.modules[1].zones.len(), 1);
        assert_eq!(report.modules[2].zones.len(), 1);
    }

    #[test]
    fn remedy_prompt_lists_modules_and_signals() {
        let modules = vec![
            RiskModule {
                path: "crates/fennec/src/agent".into(),
                unreviewed: 1,
                untested: 2,
                churning: 1,
                zones: vec![
                    RiskZone {
                        path: "crates/fennec/src/agent/turn.rs".into(),
                        signals: vec![Signal::Unreviewed, Signal::Untested, Signal::Churn],
                        churn: 12,
                        lines: 100,
                        unreviewed_percent: 100,
                    },
                    RiskZone {
                        path: "crates/fennec/src/agent/round.rs".into(),
                        signals: vec![Signal::Untested],
                        churn: 0,
                        lines: 40,
                        unreviewed_percent: 0,
                    },
                ],
                remedy_prompt: String::new(),
            },
            RiskModule {
                path: "web/src".into(),
                unreviewed: 1,
                untested: 0,
                churning: 0,
                zones: vec![RiskZone {
                    path: "web/src/app.tsx".into(),
                    signals: vec![Signal::Unreviewed],
                    churn: 0,
                    lines: 20,
                    unreviewed_percent: 100,
                }],
                remedy_prompt: String::new(),
            },
        ];

        let expected = format!(
            "Remediate the risk zones of this project, measured from git, riskiest module first. Signals: unreviewed = more than half of its lines come from AI commits never integrated through fennec; untested = no inline test and no test file of the same stem; churn = touched by {HOT_CHURN} commits or more in the last {CHURN_DAYS} days.\n\n## crates/fennec/src/agent\n- crates/fennec/src/agent/turn.rs: unreviewed, untested, churn (12 commits)\n- crates/fennec/src/agent/round.rs: untested\n\n## web/src\n- web/src/app.tsx: unreviewed\n"
        );

        assert_eq!(remedy_prompt(&modules), expected);
    }

    #[test]
    fn remedy_prompt_is_empty_without_zones() {
        assert_eq!(remedy_prompt(&[]), "");
    }

    #[test]
    fn is_unreviewed_file_uses_majority_threshold() {
        let majority = FileFacts {
            path: "majority.rs".into(),
            lines: 10,
            unreviewed_lines: 6,
            churn: 0,
            tested: true,
        };
        let half = FileFacts {
            path: "half.rs".into(),
            lines: 10,
            unreviewed_lines: 5,
            churn: 0,
            tested: true,
        };

        assert!(is_unreviewed_file(&majority));
        assert!(!is_unreviewed_file(&half));
    }

    #[test]
    fn module_remedy_prompt_is_scoped_to_its_module() {
        let files = vec![
            FileFacts {
                path: "src/alpha/one.rs".into(),
                lines: 10,
                unreviewed_lines: 6,
                churn: 0,
                tested: true,
            },
            FileFacts {
                path: "src/beta/two.rs".into(),
                lines: 10,
                unreviewed_lines: 6,
                churn: 0,
                tested: true,
            },
        ];

        let report = report(files, History::default());

        let alpha = report
            .modules
            .iter()
            .find(|module| module.path == "src/alpha")
            .expect("alpha module");
        let beta = report
            .modules
            .iter()
            .find(|module| module.path == "src/beta")
            .expect("beta module");

        assert!(alpha.remedy_prompt.contains("src/alpha/one.rs"));
        assert!(!alpha.remedy_prompt.contains("src/beta/two.rs"));
        assert!(beta.remedy_prompt.contains("src/beta/two.rs"));
        assert!(!beta.remedy_prompt.contains("src/alpha/one.rs"));
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
