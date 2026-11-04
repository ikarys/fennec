//! Pure parsing and reporting types for integration-gate security scans.
use serde::Serialize;
use serde_json::Value;

/// One scanner hit on the branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityFinding {
    pub scanner: String,
    pub rule: String,
    pub path: String,
    pub line: Option<u32>,
    pub detail: String,
}

/// What happened to one scanner on this branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ScanState {
    Ran {
        findings: usize,
    },
    NotInstalled,
    NotApplicable,
    Failed {
        reason: String,
    },
    /// Still running in the background, for that many seconds.
    Running {
        seconds: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStatus {
    pub scanner: String,
    #[serde(flatten)]
    pub state: ScanState,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub findings: Vec<SecurityFinding>,
    pub scans: Vec<ScanStatus>,
}

impl ScanReport {
    /// A scanner still runs: its findings are not in yet.
    pub fn running(&self) -> bool {
        self.scans
            .iter()
            .any(|scan| matches!(scan.state, ScanState::Running { .. }))
    }
}

fn truncate(s: &str, max: usize) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i == max {
            out.push('…');
            break;
        }
        out.push(c);
    }
    out
}

pub fn parse_gitleaks(json: &str) -> Result<Vec<SecurityFinding>, String> {
    if json.trim().is_empty() {
        return Ok(Vec::new());
    }

    let value: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let items = value
        .as_array()
        .ok_or_else(|| "gitleaks report must be a JSON array".to_string())?;

    let mut findings = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let obj = item
            .as_object()
            .ok_or_else(|| format!("gitleaks entry {i} is not an object"))?;

        let rule = obj
            .get("RuleID")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("gitleaks entry {i} missing RuleID"))?;
        let description = obj
            .get("Description")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("gitleaks entry {i} missing Description"))?;
        let file = obj
            .get("File")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("gitleaks entry {i} missing File"))?;
        let commit = obj
            .get("Commit")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("gitleaks entry {i} missing Commit"))?;

        let line = obj
            .get("StartLine")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok());

        let commit8: String = commit.chars().take(8).collect();
        let detail = format!("{} (commit {commit8})", truncate(description, 160));

        findings.push(SecurityFinding {
            scanner: "gitleaks".to_string(),
            rule: rule.to_string(),
            path: file.to_string(),
            line,
            detail,
        });
    }

    Ok(findings)
}

pub fn parse_cargo_audit(json: &str, lockfile: &str) -> Result<Vec<SecurityFinding>, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let list = value
        .as_object()
        .and_then(|v| v.get("vulnerabilities"))
        .and_then(Value::as_object)
        .and_then(|v| v.get("list"))
        .and_then(Value::as_array)
        .ok_or_else(|| "cargo audit report must have vulnerabilities.list".to_string())?;

    let mut findings = Vec::new();
    for (i, item) in list.iter().enumerate() {
        let obj = item
            .as_object()
            .ok_or_else(|| format!("cargo audit entry {i} is not an object"))?;

        let advisory = obj
            .get("advisory")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("cargo audit entry {i} missing advisory"))?;
        let id = advisory
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("cargo audit entry {i} missing advisory.id"))?;
        let title = advisory
            .get("title")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("cargo audit entry {i} missing advisory.title"))?;

        let package = obj
            .get("package")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("cargo audit entry {i} missing package"))?;
        let name = package
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("cargo audit entry {i} missing package.name"))?;
        let version = package
            .get("version")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("cargo audit entry {i} missing package.version"))?;

        findings.push(SecurityFinding {
            scanner: "cargo-audit".to_string(),
            rule: id.to_string(),
            path: lockfile.to_string(),
            line: None,
            detail: format!("{name} {version}: {title}"),
        });
    }

    Ok(findings)
}

pub fn parse_semgrep(json: &str) -> Result<Vec<SecurityFinding>, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let results = value
        .as_object()
        .and_then(|v| v.get("results"))
        .and_then(Value::as_array)
        .ok_or_else(|| "semgrep report must have results".to_string())?;

    let mut findings = Vec::new();
    for (i, item) in results.iter().enumerate() {
        let obj = item
            .as_object()
            .ok_or_else(|| format!("semgrep result {i} is not an object"))?;

        let check_id = obj
            .get("check_id")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("semgrep result {i} missing check_id"))?;
        let path = obj
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("semgrep result {i} missing path"))?;
        let line = obj
            .get("start")
            .and_then(Value::as_object)
            .and_then(|s| s.get("line"))
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| format!("semgrep result {i} missing start.line"))?;

        let extra = obj
            .get("extra")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("semgrep result {i} missing extra"))?;
        let message = extra
            .get("message")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("semgrep result {i} missing extra.message"))?;
        let severity = extra
            .get("severity")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("semgrep result {i} missing extra.severity"))?;

        findings.push(SecurityFinding {
            scanner: "semgrep".to_string(),
            rule: check_id.to_string(),
            path: path.to_string(),
            line: Some(line),
            detail: truncate(&format!("{severity}: {message}"), 160),
        });
    }

    Ok(findings)
}

pub fn changed_lockfiles(files: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|f| *f == "Cargo.lock" || f.ends_with("/Cargo.lock"))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gitleaks_hit() {
        let json = r#"[
            {
                "RuleID": "aws-key",
                "Description": "AWS key",
                "File": "a.txt",
                "StartLine": 3,
                "Commit": "abcdef1234567890",
                "Secret": "supersecret"
            }
        ]"#;

        let findings = parse_gitleaks(json).unwrap();
        assert_eq!(findings.len(), 1);

        let f = &findings[0];
        assert_eq!(f.scanner, "gitleaks");
        assert_eq!(f.rule, "aws-key");
        assert_eq!(f.path, "a.txt");
        assert_eq!(f.line, Some(3));
        assert!(f.detail.starts_with("AWS key"));
        assert!(f.detail.contains("commit abcdef12"));
        assert!(!f.detail.contains("supersecret"));
    }

    #[test]
    fn parse_gitleaks_empty() {
        assert!(parse_gitleaks("[]").unwrap().is_empty());
        assert!(parse_gitleaks("   \n").unwrap().is_empty());
    }

    #[test]
    fn parse_gitleaks_malformed() {
        assert!(parse_gitleaks("{}").is_err());
        assert!(parse_gitleaks(r#"[{"RuleID":"x"}]"#).is_err());
    }

    #[test]
    fn parse_cargo_audit_hit() {
        let json = r#"{
            "vulnerabilities": {
                "list": [
                    {
                        "advisory": {"id": "RUSTSEC-2023-0001", "title": "Bad"},
                        "package": {"name": "x", "version": "1.2.3"}
                    }
                ]
            }
        }"#;

        let findings = parse_cargo_audit(json, "Cargo.lock").unwrap();
        assert_eq!(findings.len(), 1);

        let f = &findings[0];
        assert_eq!(f.scanner, "cargo-audit");
        assert_eq!(f.rule, "RUSTSEC-2023-0001");
        assert_eq!(f.path, "Cargo.lock");
        assert_eq!(f.line, None);
        assert_eq!(f.detail, "x 1.2.3: Bad");
    }

    #[test]
    fn parse_cargo_audit_empty_report() {
        let json = r#"{"vulnerabilities":{"list":[]}}"#;
        assert!(parse_cargo_audit(json, "Cargo.lock").unwrap().is_empty());
    }

    #[test]
    fn parse_cargo_audit_malformed() {
        assert!(parse_cargo_audit("", "Cargo.lock").is_err());
        assert!(parse_cargo_audit("{}", "Cargo.lock").is_err());
    }

    #[test]
    fn parse_semgrep_hit() {
        let json = r#"{
            "results": [
                {
                    "check_id": "python.lang.correctness",
                    "path": "src/a.py",
                    "start": {"line": 3},
                    "extra": {"message": "bad code", "severity": "ERROR"}
                }
            ]
        }"#;

        let findings = parse_semgrep(json).unwrap();
        assert_eq!(findings.len(), 1);

        let f = &findings[0];
        assert_eq!(f.scanner, "semgrep");
        assert_eq!(f.rule, "python.lang.correctness");
        assert_eq!(f.path, "src/a.py");
        assert_eq!(f.line, Some(3));
        assert_eq!(f.detail, "ERROR: bad code");
    }

    #[test]
    fn parse_semgrep_empty_report() {
        let json = r#"{"results":[]}"#;
        assert!(parse_semgrep(json).unwrap().is_empty());
    }

    #[test]
    fn parse_semgrep_malformed() {
        assert!(parse_semgrep("").is_err());
        assert!(parse_semgrep("[]").is_err());
    }

    #[test]
    fn changed_lockfiles_selects_only_cargo_lockfiles() {
        let files = vec![
            "Cargo.lock".to_string(),
            "crates/x/Cargo.lock".to_string(),
            "src/main.rs".to_string(),
        ];

        assert_eq!(
            changed_lockfiles(&files),
            vec!["Cargo.lock".to_string(), "crates/x/Cargo.lock".to_string()]
        );
    }
}
