//! An audit report (the audit agents' fixed format): its findings parsed from the model's answer, and the backlog task each one becomes. fennec-only, no openfox counterpart.

use serde::Serialize;

use crate::domain::event::{Event, Role};

pub const MAX_TASK_CHARS: usize = 4_000;

/// Finding numbers already turned into tasks since the last assistant answer.
///
/// A later assistant answer starts a new audit report, so only creations after
/// the last assistant message refer to the report the UI is looking at.
pub fn taken(events: &[Event]) -> Vec<u32> {
    let mut last_assistant = None;
    for (index, event) in events.iter().enumerate() {
        if matches!(
            event,
            Event::MessageStart {
                role: Role::Assistant,
                ..
            }
        ) {
            last_assistant = Some(index);
        }
    }
    let mut numbers = Vec::new();
    if let Some(start) = last_assistant {
        for event in &events[start + 1..] {
            if let Event::AuditTasksCreated { findings, .. } = event {
                numbers.extend(findings.iter().copied());
            }
        }
    }
    numbers.sort_unstable();
    numbers.dedup();
    numbers
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub number: u32,
    pub title: String,
    pub severity: Severity,
    pub principle: String,
    pub locations: Vec<String>,
    pub problem: String,
    pub fix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Audit {
    pub scope: String,
    pub summary: String,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Severity,
    Principle,
    Where,
    Problem,
    Fix,
}

struct FindingHeading {
    number: u32,
    title: String,
}

struct FieldLine {
    key: Field,
    value: String,
}

pub fn parse(text: &str) -> Option<Audit> {
    let mut lines = text.lines();
    let heading = lines.find(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("## Audit") || trimmed.starts_with("### Audit")
    })?;
    let heading = heading.trim();
    let scope = heading
        .split_once(':')
        .map(|(_, scope)| scope.trim().to_string())
        .unwrap_or_default();

    let mut summary_lines = Vec::new();
    let mut findings = Vec::new();
    let mut current: Option<Finding> = None;
    let mut last_field: Option<Field> = None;
    let mut in_summary = true;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(heading) = finding_heading(trimmed) {
            if let Some(finding) = current.take() {
                findings.push(finding);
            }
            last_field = None;
            in_summary = false;
            if !heading.title.is_empty() {
                current = Some(Finding {
                    number: heading.number,
                    title: heading.title,
                    severity: Severity::Medium,
                    principle: String::new(),
                    locations: Vec::new(),
                    problem: String::new(),
                    fix: String::new(),
                });
            }
            continue;
        }

        if trimmed.starts_with("## ") {
            break;
        }

        if in_summary {
            summary_lines.push(trimmed.to_string());
            continue;
        }

        if let Some(field) = field_line(trimmed) {
            if let Some(finding) = current.as_mut() {
                match field.key {
                    Field::Severity => {
                        finding.severity = parse_severity(&field.value);
                        last_field = Some(Field::Severity);
                    }
                    Field::Principle => {
                        finding.principle = field.value;
                        last_field = Some(Field::Principle);
                    }
                    Field::Where => {
                        finding.locations = parse_locations(&field.value);
                        last_field = Some(Field::Where);
                    }
                    Field::Problem => {
                        finding.problem = field.value;
                        last_field = Some(Field::Problem);
                    }
                    Field::Fix => {
                        finding.fix = field.value;
                        last_field = Some(Field::Fix);
                    }
                }
            }
            continue;
        }

        if let (Some(finding), Some(field)) = (current.as_mut(), last_field) {
            match field {
                Field::Severity => {
                    let current = severity_word(finding.severity);
                    finding.severity = parse_severity(&format!("{current} {trimmed}"));
                }
                Field::Principle => finding.principle.push_str(&format!(" {trimmed}")),
                Field::Where => {
                    finding.locations =
                        parse_locations(&format!("{} {}", finding.locations.join(", "), trimmed));
                }
                Field::Problem => finding.problem.push_str(&format!(" {trimmed}")),
                Field::Fix => finding.fix.push_str(&format!(" {trimmed}")),
            }
        }
    }

    if let Some(finding) = current {
        findings.push(finding);
    }

    Some(Audit {
        scope,
        summary: summary_lines.join("\n"),
        findings,
    })
}

impl Audit {
    pub fn task_text(&self, finding: &Finding) -> String {
        let mut parts = vec![finding.title.clone()];

        if !finding.problem.is_empty() {
            parts.push(finding.problem.clone());
        }
        if !finding.fix.is_empty() {
            parts.push(format!("Fix: {}", finding.fix));
        }
        if !finding.locations.is_empty() {
            parts.push(format!("Where: {}", finding.locations.join(", ")));
        }
        if !finding.principle.is_empty() {
            parts.push(format!(
                "Principle: {} · severity {}",
                finding.principle,
                severity_word(finding.severity)
            ));
        }
        parts.push(format!("From the architecture audit \"{}\".", self.scope));

        parts.join("\n\n").chars().take(MAX_TASK_CHARS).collect()
    }
}

fn finding_heading(trimmed: &str) -> Option<FindingHeading> {
    let rest = trimmed
        .strip_prefix("## ")
        .or_else(|| trimmed.strip_prefix("### "))?;
    if !rest.to_lowercase().starts_with("finding ") {
        return None;
    }

    let after = &rest["Finding ".len()..];
    let colon = after.find(':')?;
    let digits = &after[..colon];
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let number = digits.parse().ok()?;
    let title = after[colon + 1..].trim().to_string();

    Some(FindingHeading { number, title })
}

fn field_line(trimmed: &str) -> Option<FieldLine> {
    let rest = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))?;
    let colon = rest.find(':')?;
    let raw_key = &rest[..colon];
    let key = raw_key.trim().trim_matches('*').trim().to_lowercase();
    let value = rest[colon + 1..].trim().to_string();

    let key = match key.as_str() {
        "severity" => Field::Severity,
        "principle" => Field::Principle,
        "where" => Field::Where,
        "problem" => Field::Problem,
        "fix" => Field::Fix,
        _ => return None,
    };

    Some(FieldLine { key, value })
}

fn parse_severity(value: &str) -> Severity {
    match value
        .split_whitespace()
        .next()
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("high") => Severity::High,
        Some("medium") => Severity::Medium,
        Some("low") => Severity::Low,
        _ => Severity::Medium,
    }
}

fn parse_locations(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|location| location.trim().trim_matches('`').trim().to_string())
        .filter(|location| !location.is_empty())
        .collect()
}

fn severity_word(severity: Severity) -> &'static str {
    match severity {
        Severity::High => "high",
        Severity::Medium => "medium",
        Severity::Low => "low",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assistant_message(id: &str) -> Event {
        serde_json::from_value(serde_json::json!({
            "type": "message.start",
            "data": { "messageId": id, "role": "assistant" }
        }))
        .unwrap()
    }

    fn audit_tasks_created(findings: Vec<u32>, idea_ids: Vec<&str>) -> Event {
        serde_json::from_value(serde_json::json!({
            "type": "audit.tasks_created",
            "data": { "findings": findings, "ideaIds": idea_ids }
        }))
        .unwrap()
    }

    #[test]
    fn taken_is_empty_without_creations() {
        let events = vec![assistant_message("m1")];
        assert!(taken(&events).is_empty());
    }

    #[test]
    fn taken_after_the_report() {
        let events = vec![
            assistant_message("m1"),
            audit_tasks_created(vec![2, 1], vec!["a", "b"]),
        ];
        assert_eq!(taken(&events), vec![1, 2]);
    }

    #[test]
    fn a_report_after_the_creation_resets_taken() {
        let events = vec![
            assistant_message("m1"),
            audit_tasks_created(vec![1], vec!["a"]),
            assistant_message("m2"),
        ];
        assert!(taken(&events).is_empty());
    }

    #[test]
    fn two_creations_are_merged() {
        let events = vec![
            assistant_message("m1"),
            audit_tasks_created(vec![1, 2], vec!["a", "b"]),
            audit_tasks_created(vec![2, 3], vec!["b", "c"]),
        ];
        assert_eq!(taken(&events), vec![1, 2, 3]);
    }

    #[test]
    fn parses_full_report_with_two_findings() {
        let text = r#"
## Audit: workspace

Summary line one.
Summary line two.

### Finding 1: Enforce domain purity
- Severity: high
- Principle: single responsibility
- Where: `src/a.rs:40`, `web/b.tsx`
- Problem: Domain touches I/O.
- Fix: Move I/O to adapters.

### Finding 2: Split controller
- Severity: low
- Principle: kiss
- Where: src/c.rs
- Problem: Too many routes.
- Fix: Extract handlers.

## Next
ignored
"#;

        let audit = parse(text).expect("report should parse");

        assert_eq!(audit.scope, "workspace");
        assert_eq!(audit.summary, "Summary line one.\nSummary line two.");
        assert_eq!(audit.findings.len(), 2);

        let first = &audit.findings[0];
        assert_eq!(first.number, 1);
        assert_eq!(first.title, "Enforce domain purity");
        assert_eq!(first.severity, Severity::High);
        assert_eq!(first.principle, "single responsibility");
        assert_eq!(first.locations, vec!["src/a.rs:40", "web/b.tsx"]);
        assert_eq!(first.problem, "Domain touches I/O.");
        assert_eq!(first.fix, "Move I/O to adapters.");

        let second = &audit.findings[1];
        assert_eq!(second.number, 2);
        assert_eq!(second.title, "Split controller");
        assert_eq!(second.severity, Severity::Low);
        assert_eq!(second.locations, vec!["src/c.rs"]);
    }

    #[test]
    fn parses_heading_drift_and_bold_keys() {
        let text = r#"
### Audit: scope

### Finding 1: Title
- **Severity**: high
- **Principle**: p
- **Where**: `a`, `b`
- **Problem**: p
- **Fix**: f
"#;

        let audit = parse(text).expect("report should parse");

        assert_eq!(audit.scope, "scope");
        assert_eq!(audit.findings.len(), 1);
        assert_eq!(audit.findings[0].severity, Severity::High);
        assert_eq!(audit.findings[0].locations, vec!["a", "b"]);
    }

    #[test]
    fn missing_or_unknown_severity_defaults_to_medium() {
        let missing = r#"
## Audit: s

### Finding 1: T
- Problem: p
"#;
        let unknown = r#"
## Audit: s

### Finding 1: T
- Severity: critical
- Problem: p
"#;

        assert_eq!(
            parse(missing)
                .expect("missing severity should parse")
                .findings[0]
                .severity,
            Severity::Medium
        );
        assert_eq!(
            parse(unknown)
                .expect("unknown severity should parse")
                .findings[0]
                .severity,
            Severity::Medium
        );
    }

    #[test]
    fn wrapped_problem_line_continues_the_field() {
        let text = r#"
## Audit: s

### Finding 1: T
- Problem: first
second line
- Fix: f
"#;

        let audit = parse(text).expect("report should parse");

        assert_eq!(audit.findings[0].problem, "first second line");
        assert_eq!(audit.findings[0].fix, "f");
    }

    #[test]
    fn no_audit_heading_is_none() {
        assert_eq!(parse("### Finding 1: T"), None);
    }

    #[test]
    fn audit_heading_without_findings_keeps_summary_only() {
        let text = r#"
## Audit: s

Summary
"#;

        let audit = parse(text).expect("report should parse");

        assert_eq!(audit.scope, "s");
        assert_eq!(audit.summary, "Summary");
        assert!(audit.findings.is_empty());
    }

    #[test]
    fn text_before_the_audit_heading_is_ignored() {
        let text = r#"
intro

## Audit: s

Summary
"#;

        let audit = parse(text).expect("report should parse");

        assert_eq!(audit.scope, "s");
        assert_eq!(audit.summary, "Summary");
    }

    #[test]
    fn task_text_includes_fields_and_omits_empty_ones() {
        let full = Finding {
            number: 1,
            title: "Title".into(),
            severity: Severity::High,
            principle: "principle".into(),
            locations: vec!["a".into(), "b".into()],
            problem: "Problem".into(),
            fix: "Fix".into(),
        };
        let audit = Audit {
            scope: "scope".into(),
            summary: String::new(),
            findings: vec![full.clone()],
        };

        let text = audit.task_text(&full);
        assert!(text.contains("Title"));
        assert!(text.contains("Problem"));
        assert!(text.contains("Fix: Fix"));
        assert!(text.contains("Where: a, b"));
        assert!(text.contains("Principle: principle · severity high"));
        assert!(text.contains("From the architecture audit \"scope\"."));

        let empty = Finding {
            number: 2,
            title: "Empty".into(),
            severity: Severity::Low,
            principle: String::new(),
            locations: Vec::new(),
            problem: String::new(),
            fix: String::new(),
        };
        let empty_audit = Audit {
            scope: "scope".into(),
            summary: String::new(),
            findings: vec![empty.clone()],
        };

        let empty_text = empty_audit.task_text(&empty);
        assert!(!empty_text.contains("Fix:"));
        assert!(!empty_text.contains("Where:"));
        assert!(!empty_text.contains("Principle:"));
        assert!(empty_text.contains("Empty"));
    }

    #[test]
    fn task_text_is_truncated_to_max_chars() {
        let finding = Finding {
            number: 1,
            title: "a".repeat(5_000),
            severity: Severity::Medium,
            principle: String::new(),
            locations: Vec::new(),
            problem: String::new(),
            fix: String::new(),
        };
        let audit = Audit {
            scope: "scope".into(),
            summary: String::new(),
            findings: vec![finding.clone()],
        };

        let text = audit.task_text(&finding);

        assert_eq!(text.chars().count(), MAX_TASK_CHARS);
    }
}
