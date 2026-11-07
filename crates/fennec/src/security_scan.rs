//! Adapter that runs the integration-gate scanners in the worktree.
use crate::domain::security_scan::{self, ScanReport, ScanState, ScanStatus, SecurityFinding};
use crate::scanners;
use crate::tools::Sandbox;
use crate::worktree::Worktree;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

// ponytail: one entry, the last gate opened; a map if several dialogs alternate.
static CACHE: Mutex<Option<Entry>> = Mutex::new(None);

struct Entry {
    key: String,
    started: Instant,
    slots: Vec<Slot>,
}

struct Slot {
    scanner: &'static str,
    done: Option<(ScanState, Vec<SecurityFinding>)>,
}

/// The gate's scan of a branch, never waiting on a scanner: the first call
/// starts them in the background, the next ones read how far they got.
pub fn scan(top: &Path, wt: &Worktree, files: &[String], sandbox: &Sandbox) -> ScanReport {
    let key = cache_key(top, wt);

    if let Some(key) = key {
        let (key, report) = {
            let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());

            if let Some(entry) = cache.as_ref()
                && entry.key == key
            {
                return snapshot(entry);
            }

            let entry = fresh_entry(key.clone());
            let report = snapshot(&entry);
            *cache = Some(entry);
            (key, report)
        };

        let top = top.to_path_buf();
        let wt = wt.clone();
        let files = files.to_vec();
        let sandbox = sandbox.clone();

        thread::spawn(move || {
            thread::scope(|s| {
                s.spawn(|| finish(&key, "gitleaks", scan_gitleaks(&top, &wt, &files, &sandbox)));
                s.spawn(|| finish(&key, "cargo-audit", scan_cargo_audit(&wt, &files, &sandbox)));
                s.spawn(|| finish(&key, "semgrep", scan_semgrep(&top, &wt, &files, &sandbox)));
            });
        });

        report
    } else {
        run_scan(top, wt, files, sandbox)
    }
}

fn fresh_entry(key: String) -> Entry {
    Entry {
        key,
        started: Instant::now(),
        slots: vec![
            Slot {
                scanner: "gitleaks",
                done: None,
            },
            Slot {
                scanner: "cargo-audit",
                done: None,
            },
            Slot {
                scanner: "semgrep",
                done: None,
            },
        ],
    }
}

fn finish(key: &str, scanner: &str, result: (ScanState, Vec<SecurityFinding>)) {
    // A newer gate may have replaced this entry while the scanner was running.
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());

    if let Some(entry) = cache.as_mut()
        && entry.key == key
        && let Some(slot) = entry.slots.iter_mut().find(|slot| slot.scanner == scanner)
    {
        slot.done = Some(result);
    }
}

fn snapshot(entry: &Entry) -> ScanReport {
    let mut findings = Vec::new();
    let mut scans = Vec::new();

    for slot in &entry.slots {
        match &slot.done {
            Some((state, slot_findings)) => {
                findings.extend(slot_findings.iter().cloned());
                scans.push(ScanStatus {
                    scanner: slot.scanner.to_string(),
                    state: state.clone(),
                });
            }
            None => scans.push(ScanStatus {
                scanner: slot.scanner.to_string(),
                state: ScanState::Running {
                    seconds: entry.started.elapsed().as_secs(),
                },
            }),
        }
    }

    ScanReport { findings, scans }
}

fn cache_key(top: &Path, wt: &Worktree) -> Option<String> {
    let base = resolve_sha(top, &wt.base)?;
    let branch = resolve_sha(top, &wt.branch)?;
    Some(format!("{}|{}|{}", top.to_string_lossy(), base, branch))
}

fn resolve_sha(top: &Path, r: &str) -> Option<String> {
    let ref_arg = format!("{r}^{{commit}}");
    let sha = crate::git::stdout(top, &["rev-parse", &ref_arg])?
        .trim()
        .to_string();
    if sha.is_empty() { None } else { Some(sha) }
}

fn run_scan(top: &Path, wt: &Worktree, files: &[String], sandbox: &Sandbox) -> ScanReport {
    let mut findings = Vec::new();
    let mut scans = Vec::new();

    let (state, mut scanner_findings) = scan_gitleaks(top, wt, files, sandbox);
    findings.append(&mut scanner_findings);
    scans.push(ScanStatus {
        scanner: "gitleaks".to_string(),
        state,
    });

    let (state, mut scanner_findings) = scan_cargo_audit(wt, files, sandbox);
    findings.append(&mut scanner_findings);
    scans.push(ScanStatus {
        scanner: "cargo-audit".to_string(),
        state,
    });

    let (state, mut scanner_findings) = scan_semgrep(top, wt, files, sandbox);
    findings.append(&mut scanner_findings);
    scans.push(ScanStatus {
        scanner: "semgrep".to_string(),
        state,
    });

    ScanReport { findings, scans }
}

fn scan_gitleaks(
    top: &Path,
    wt: &Worktree,
    files: &[String],
    sandbox: &Sandbox,
) -> (ScanState, Vec<SecurityFinding>) {
    if files.is_empty() {
        return (ScanState::NotApplicable, Vec::new());
    }

    let Some(program) = scanners::find("gitleaks") else {
        return (ScanState::NotInstalled, Vec::new());
    };

    let args = vec![
        "git".to_string(),
        // The gate's own start: after a merge by hand, the commits already in HEAD are not rescanned.
        format!(
            "--log-opts={start}..{branch}",
            start = crate::integration::merge_start(top, wt),
            branch = wt.branch
        ),
        "--report-format".to_string(),
        "json".to_string(),
        "--report-path".to_string(),
        "-".to_string(),
        "--redact".to_string(),
        "--no-banner".to_string(),
        "--exit-code".to_string(),
        "0".to_string(),
    ];

    match run(&program, &args, top, top, sandbox, &[]) {
        Ok(stdout) => match security_scan::parse_gitleaks(&stdout) {
            Ok(found) => (
                ScanState::Ran {
                    findings: found.len(),
                },
                found,
            ),
            Err(e) => (
                ScanState::Failed {
                    reason: short_reason(&e),
                },
                Vec::new(),
            ),
        },
        Err(e) => (
            ScanState::Failed {
                reason: short_reason(&e),
            },
            Vec::new(),
        ),
    }
}

fn scan_cargo_audit(
    wt: &Worktree,
    files: &[String],
    sandbox: &Sandbox,
) -> (ScanState, Vec<SecurityFinding>) {
    let lockfiles = security_scan::changed_lockfiles(files);
    if lockfiles.is_empty() {
        return (ScanState::NotApplicable, Vec::new());
    }

    let Some(program) = scanners::find("cargo-audit") else {
        return (ScanState::NotInstalled, Vec::new());
    };

    let mut findings = Vec::new();
    for lockfile in &lockfiles {
        let file = wt.path.join(lockfile);
        let args = vec![
            "audit".to_string(),
            "--no-fetch".to_string(),
            "--stale".to_string(),
            "--json".to_string(),
            "--file".to_string(),
            file.to_string_lossy().into_owned(),
        ];

        match run(&program, &args, &wt.path, &wt.path, sandbox, &[]) {
            Ok(stdout) => match security_scan::parse_cargo_audit(&stdout, lockfile) {
                Ok(mut found) => findings.append(&mut found),
                Err(e) => {
                    return (
                        ScanState::Failed {
                            reason: short_reason(&e),
                        },
                        Vec::new(),
                    );
                }
            },
            Err(e) => {
                return (
                    ScanState::Failed {
                        reason: short_reason(&e),
                    },
                    Vec::new(),
                );
            }
        }
    }

    (
        ScanState::Ran {
            findings: findings.len(),
        },
        findings,
    )
}

fn scan_semgrep(
    top: &Path,
    wt: &Worktree,
    files: &[String],
    sandbox: &Sandbox,
) -> (ScanState, Vec<SecurityFinding>) {
    let Some(cfg) = semgrep_config(top) else {
        return (ScanState::NotApplicable, Vec::new());
    };

    let targets: Vec<String> = files
        .iter()
        .filter(|f| wt.path.join(f.as_str()).exists())
        .cloned()
        .collect();

    if targets.is_empty() {
        return (ScanState::NotApplicable, Vec::new());
    }

    let Some(program) = scanners::find("semgrep") else {
        return (ScanState::NotInstalled, Vec::new());
    };

    let mut args = vec![
        "scan".to_string(),
        "--config".to_string(),
        cfg.to_string_lossy().into_owned(),
        "--json".to_string(),
        "--metrics=off".to_string(),
        "--quiet".to_string(),
        "--disable-version-check".to_string(),
    ];
    args.extend(targets);

    let env = [
        ("SEMGREP_SETTINGS_FILE", "/tmp/fennec-semgrep-settings.yml"),
        ("SEMGREP_LOG_FILE", "/tmp/fennec-semgrep.log"),
        ("SEMGREP_VERSION_CACHE_PATH", "/tmp/fennec-semgrep-version"),
    ];

    match run(&program, &args, &wt.path, &wt.path, sandbox, &env) {
        Ok(stdout) => match security_scan::parse_semgrep(&stdout) {
            Ok(found) => (
                ScanState::Ran {
                    findings: found.len(),
                },
                found,
            ),
            Err(e) => (
                ScanState::Failed {
                    reason: short_reason(&e),
                },
                Vec::new(),
            ),
        },
        Err(e) => (
            ScanState::Failed {
                reason: short_reason(&e),
            },
            Vec::new(),
        ),
    }
}

fn semgrep_config(top: &Path) -> Option<PathBuf> {
    for name in [".semgrep.yml", ".semgrep.yaml", ".semgrep"] {
        let path = top.join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn short_reason(reason: &str) -> String {
    let first = reason.lines().next().unwrap_or("");
    first.chars().take(200).collect()
}

fn run(
    program: &Path,
    args: &[String],
    workdir: &Path,
    cwd: &Path,
    sandbox: &Sandbox,
    env: &[(&str, &str)],
) -> Result<String, String> {
    let program_str = program
        .to_str()
        .ok_or_else(|| "scanner path is not valid unicode".to_string())?;

    // Off: the bare program, its secret env names still stripped.
    let mut command = sandbox
        .read_only(workdir)
        .as_ref()
        .unwrap_or(sandbox)
        .spawn(program_str, args, workdir, cwd);

    command
        .envs(env.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "stdout pipe missing".to_string())?;

    let reader = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });

    let deadline = Instant::now() + Duration::from_secs(120);

    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let bytes = reader
                    .join()
                    .map_err(|_| "stdout reader panicked".to_string())?;
                return Ok(String::from_utf8_lossy(&bytes).into_owned());
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = reader.join();
                    return Err("timed out after 120 s".to_string());
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        let dir = std::env::temp_dir().join(format!(
            "fennec-security-scan-test-{tag}-{}-{nanos}",
            std::process::id()
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn semgrep_config_prefers_yml() {
        let dir = temp_dir("prefers-yml");
        fs::write(dir.join(".semgrep.yaml"), "rules: []").unwrap();
        fs::write(dir.join(".semgrep.yml"), "rules: []").unwrap();

        assert_eq!(semgrep_config(&dir), Some(dir.join(".semgrep.yml")));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn semgrep_config_uses_dir_when_no_files() {
        let dir = temp_dir("dir");
        fs::create_dir_all(dir.join(".semgrep")).unwrap();

        assert_eq!(semgrep_config(&dir), Some(dir.join(".semgrep")));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn semgrep_config_missing() {
        let dir = temp_dir("missing");
        assert_eq!(semgrep_config(&dir), None);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn short_reason_uses_first_line_and_limits_length() {
        let long = "a".repeat(300);
        let reason = format!("{long}\nsecond line");

        assert_eq!(short_reason(&reason), "a".repeat(200));
        assert_eq!(short_reason("first\nsecond"), "first");
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.email=t@t", "-c", "user.name=t"])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    /// The real scanner in the real sandbox: a read-only world must still let
    /// gitleaks read the repository and write its report.
    #[test]
    fn gitleaks_flags_a_secret_committed_on_the_branch_in_the_background_sandbox() {
        if scanners::find("gitleaks").is_none()
            || !matches!(crate::tools::probe(), crate::tools::SandboxHealth::Ready)
        {
            return;
        }
        let top = temp_dir("gitleaks-e2e");
        git(&top, &["init", "-q", "-b", "main"]);
        fs::write(top.join("a.txt"), "a\n").unwrap();
        git(&top, &["add", "."]);
        git(&top, &["commit", "-qm", "base"]);
        git(&top, &["update-ref", "refs/fennec/base/t", "HEAD"]);
        git(&top, &["switch", "-qc", "fennec/t"]);
        fs::write(
            top.join("s.py"),
            "token = \"ghp_8k2LmQ9vXr4TzW7pN1bYc6JdHs3FgA0eKuRi\"\n",
        )
        .unwrap();
        git(&top, &["add", "."]);
        git(&top, &["commit", "-qm", "secret"]);
        // The human is back on their branch: the gate starts from where it forks.
        git(&top, &["switch", "-q", "main"]);
        let wt = Worktree {
            path: top.clone(),
            workdir: top.clone(),
            branch: "fennec/t".into(),
            base: "refs/fennec/base/t".into(),
            origin: String::new(),
        };
        let sandbox = Sandbox::from_prefs(
            &crate::domain::preferences::SandboxPrefs {
                enabled: true,
                network: false,
            },
            None,
            Path::new(""),
        );

        let files = ["s.py".to_string()];

        // The first call answers at once, every scanner still running.
        let first = scan(&top, &wt, &files, &sandbox);
        assert!(
            first
                .scans
                .iter()
                .all(|s| matches!(s.state, ScanState::Running { .. }))
        );
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut report = scan(&top, &wt, &files, &sandbox);
        while report.running() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
            report = scan(&top, &wt, &files, &sandbox);
        }

        assert_eq!(
            report.scans[0].state,
            ScanState::Ran { findings: 1 },
            "{report:?}"
        );
        assert_eq!(report.findings[0].path, "s.py");
        assert!(!format!("{report:?}").contains("ghp_8k2LmQ"));
        let _ = fs::remove_dir_all(&top);
    }
}
