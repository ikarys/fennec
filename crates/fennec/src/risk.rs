//! Git-backed adapter that measures project risk zones from a workdir.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;
use std::thread::{available_parallelism, scope};

use crate::domain::indicators::Authored;
use crate::domain::module_map::SourceFile;
use crate::domain::risk::{self, Authorship, FileFacts, History, RiskReport};

pub fn load(workdir: &Path) -> RiskReport {
    let sources = crate::module_map::tracked_files(workdir);

    let Some(ai_log) = git_stdout(workdir, &["log", "--format=%x1e%H%x1f%B"]) else {
        return risk::report(Vec::new(), History::default());
    };
    let (ai_hashes, commit_count) = ai_commits(&ai_log);
    let history = History {
        commits: commit_count,
        ai_commits: ai_hashes.len(),
    };

    let since = churn_since();
    let Some(churn_log) = git_stdout(workdir, &["log", &since, "--format=%x1e", "--name-only"])
    else {
        return risk::report(Vec::new(), History::default());
    };
    let churn = churn_counts(&churn_log);

    let mut test_paths: Vec<String> = Vec::new();
    let mut candidates: Vec<SourceFile> = Vec::new();

    for source in sources {
        if crate::domain::test_files::is_test_path(&source.path) {
            test_paths.push(source.path);
        } else {
            candidates.push(source);
        }
    }

    let test_paths: Vec<&str> = test_paths.iter().map(|path| path.as_str()).collect();
    let workers = available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);

    // One chunk per core: blaming the whole repository is the slow part.
    let chunk_size = candidates.len().div_ceil(workers).max(1);
    let facts: Vec<FileFacts> = scope(|s| {
        let handles: Vec<_> = candidates
            .chunks(chunk_size)
            .map(|chunk| {
                let (ai_hashes, churn, test_paths) = (&ai_hashes, &churn, &test_paths);
                s.spawn(move || measure_chunk(workdir, chunk, ai_hashes, churn, test_paths))
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("risk worker panicked"))
            .collect()
    });

    risk::report(facts, history)
}

/// The `--since` argument shared by the risk report and the hot-file lookup.
fn churn_since() -> String {
    format!("--since={}.days", risk::CHURN_DAYS)
}

/// Repository-top-relative paths with at least `risk::HOT_CHURN` commits in
/// the last `risk::CHURN_DAYS` days; empty when git cannot answer.
pub fn hot_files(top: &Path) -> HashSet<String> {
    let since = churn_since();
    let Some(log) = git_stdout(top, &["log", &since, "--format=%x1e", "--name-only"]) else {
        return HashSet::new();
    };

    churn_counts(&log)
        .into_iter()
        .filter_map(|(path, count)| (count >= risk::HOT_CHURN).then_some(path))
        .collect()
}

/// The commits of the last `days` days by authorship, read from their trailers.
pub fn recent_authorship(workdir: &Path, days: u32) -> Authored {
    let since = format!("--since={days}.days");
    let Some(log) = git_stdout(workdir, &["log", &since, "--format=%x1e%B"]) else {
        return Authored::default();
    };

    let mut authored = Authored::default();

    for chunk in log.split('\x1e') {
        if chunk.trim().is_empty() {
            continue;
        }
        match risk::authorship(chunk) {
            Authorship::Human => authored.human += 1,
            Authorship::Integrated => authored.integrated += 1,
            Authorship::Ai => authored.ai += 1,
        }
    }

    authored
}

/// Of `files` (top-relative), the ones that are tested on `branch`.
///
/// A file whose content cannot be read from the branch is treated as tested:
/// it is then out of the way and cannot carry an `Untested` review signal.
pub fn tested_on_branch(top: &Path, branch: &str, files: &[String]) -> HashSet<String> {
    let tree = git_stdout(top, &["ls-tree", "-r", "--name-only", branch]).unwrap_or_default();
    let test_paths: Vec<String> = tree
        .lines()
        .map(str::trim)
        .filter(|path| !path.is_empty() && crate::domain::test_files::is_test_path(path))
        .map(String::from)
        .collect();
    let test_refs: Vec<&str> = test_paths.iter().map(String::as_str).collect();

    let mut tested = HashSet::new();

    for file in files {
        let show = format!("{branch}:{file}");
        let Some(content) = git_stdout(top, &["show", &show]) else {
            tested.insert(file.clone());
            continue;
        };

        if risk::is_tested(file, &content, &test_refs) {
            tested.insert(file.clone());
        }
    }

    tested
}

fn measure_chunk(
    workdir: &Path,
    sources: &[SourceFile],
    ai_hashes: &HashSet<String>,
    churn: &HashMap<String, usize>,
    test_paths: &[&str],
) -> Vec<FileFacts> {
    sources
        .iter()
        .map(|source| measure_source(workdir, source, ai_hashes, churn, test_paths))
        .collect()
}

fn measure_source(
    workdir: &Path,
    source: &SourceFile,
    ai_hashes: &HashSet<String>,
    churn: &HashMap<String, usize>,
    test_paths: &[&str],
) -> FileFacts {
    let (lines, unreviewed_lines) = git_blame_counts(workdir, &source.path, ai_hashes)
        .unwrap_or_else(|| (source.content.lines().count(), 0));

    FileFacts {
        path: source.path.clone(),
        lines,
        unreviewed_lines,
        churn: churn.get(&source.path).copied().unwrap_or(0),
        tested: risk::is_tested(&source.path, &source.content, test_paths),
    }
}

fn ai_commits(log: &str) -> (HashSet<String>, usize) {
    let mut ai_hashes = HashSet::new();
    let mut commits = 0;

    for record in log.split('\x1e') {
        let Some((hash, message)) = record.split_once('\x1f') else {
            continue;
        };

        let hash = hash.trim();
        if hash.is_empty() {
            continue;
        }

        commits += 1;
        if risk::authorship(message) == Authorship::Ai {
            ai_hashes.insert(hash.to_string());
        }
    }

    (ai_hashes, commits)
}

fn churn_counts(log: &str) -> HashMap<String, usize> {
    let mut counts = HashMap::new();

    for record in log.split('\x1e') {
        for line in record.lines() {
            let path = line.trim();
            if path.is_empty() {
                continue;
            }

            *counts.entry(path.to_string()).or_insert(0) += 1;
        }
    }

    counts
}

fn blame_counts(porcelain: &str, ai_hashes: &HashSet<String>) -> (usize, usize) {
    let mut lines = 0;
    let mut unreviewed_lines = 0;

    for line in porcelain.lines() {
        if line.starts_with('\t') {
            continue;
        }

        let Some((hash, _)) = line.split_once(' ') else {
            continue;
        };

        if is_commit_hash(hash) {
            lines += 1;
            if ai_hashes.contains(hash) {
                unreviewed_lines += 1;
            }
        }
    }

    (lines, unreviewed_lines)
}

fn git_blame_counts(
    workdir: &Path,
    path: &str,
    ai_hashes: &HashSet<String>,
) -> Option<(usize, usize)> {
    let output = Command::new("git")
        .current_dir(workdir)
        .args(["blame", "--line-porcelain", "-w", "HEAD", "--", path])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Some(blame_counts(&stdout, ai_hashes))
}

fn git_stdout(workdir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(workdir)
        .args(args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn is_commit_hash(token: &str) -> bool {
    token.len() == 40 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_commits_keeps_only_ai_hashes() {
        let human = "b".repeat(40);
        let ai = "c".repeat(40);
        let integrated = "d".repeat(40);

        let log = format!(
            "\x1e{human}\x1fHuman commit\n\x1e{ai}\x1fAI commit\n\nCo-Authored-By: Claude <bot@example.com>\n\x1e{integrated}\x1fIntegrated\n\nFennec-Session: 1\nCo-Authored-By: Claude <bot@example.com>"
        );

        let (hashes, commits) = ai_commits(&log);

        assert_eq!(commits, 3);
        assert_eq!(hashes.len(), 1);
        assert!(hashes.contains(&ai));
    }

    #[test]
    fn churn_counts_counts_paths_per_commit() {
        let log = "\x1e\nsrc/a.rs\nsrc/b.rs\n\x1e\nsrc/a.rs\n";
        let counts = churn_counts(log);

        assert_eq!(counts.get("src/a.rs"), Some(&2));
        assert_eq!(counts.get("src/b.rs"), Some(&1));
    }

    #[test]
    fn blame_counts_counts_ai_and_human_headers() {
        let ai_hash = "a".repeat(40);
        let human_hash = "b".repeat(40);
        let ai_hashes = HashSet::from([ai_hash.clone()]);

        let porcelain = format!(
            "{ai_hash} 1 1 1\n\tfirst\nauthor someone\n{human_hash} 2 2 1\n\tsecond\nauthor someone\n"
        );

        let (lines, unreviewed_lines) = blame_counts(&porcelain, &ai_hashes);

        assert_eq!((lines, unreviewed_lines), (2, 1));
    }

    #[test]
    fn load_measures_this_repository() {
        let workdir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = load(&workdir);

        assert!(report.files > 0);
    }
}
