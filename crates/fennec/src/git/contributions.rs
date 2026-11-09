//! The branches that could come in as a contribution: commits the human's HEAD lacks, not already squashed into it.

use std::collections::HashSet;
use std::path::Path;

use super::{current_branch, run};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contribution {
    pub branch: String,
    pub ahead: usize,
    pub subject: String,
}

/// The branches, local and remote, holding commits the human's HEAD lacks: what
/// could come in as a contribution.
pub fn contributions(workdir: &Path) -> Vec<Contribution> {
    let out = run(
        workdir,
        &[
            "for-each-ref",
            "--no-merged",
            "HEAD",
            "--sort=-committerdate",
            // lstrip=2, not :short: `refs/remotes/origin/HEAD` stays `origin/HEAD`
            // (short gives a bare `origin`), so the filter can drop it.
            "--format=%(refname:lstrip=2)%09%(subject)",
            "refs/heads",
            "refs/remotes",
        ],
        None,
    )
    .ok()
    .map(|o| String::from_utf8_lossy(&o).into_owned())
    .unwrap_or_default();
    let current = current_branch(workdir);
    let integrated = integrated_patch_ids(workdir);
    let mut contributions = Vec::new();
    for (branch, subject) in parse_contributions(&out, current.as_deref()) {
        if is_integrated(workdir, &branch, &integrated) {
            continue;
        }
        let range = format!("HEAD..{branch}");
        let Some(ahead) = run(workdir, &["rev-list", "--count", &range], None)
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o).trim().parse::<usize>().ok())
        else {
            continue;
        };
        contributions.push(Contribution {
            branch,
            ahead,
            subject,
        });
        if contributions.len() == 50 {
            // ponytail: one rev-list per branch, capped at 50; a single
            // `for-each-ref %(ahead-behind:HEAD)` (git 2.41+) if it ever matters.
            break;
        }
    }
    contributions
}

/// Drops the session branches (fennec's own), remote HEADs, long-lived/bot
/// branches and the branch the human is on.
fn parse_contributions(out: &str, current: Option<&str>) -> Vec<(String, String)> {
    out.lines()
        .filter_map(|line| {
            let (branch, subject) = line.split_once('\t')?;
            let branch = branch.trim();
            let subject = subject.trim();
            if branch.is_empty() {
                return None;
            }
            if branch.starts_with("fennec/") || branch.contains("/fennec/") {
                return None;
            }
            if branch.ends_with("/HEAD") {
                return None;
            }
            if is_long_lived(branch) {
                return None;
            }
            if current.is_some_and(|c| c == branch) {
                return None;
            }
            Some((branch.to_string(), subject.to_string()))
        })
        .collect()
}

fn is_long_lived(branch: &str) -> bool {
    branch == "main"
        || branch == "master"
        || branch == "develop"
        || branch.ends_with("/main")
        || branch.ends_with("/master")
        || branch.ends_with("/develop")
        || branch.contains("release-please--")
}

fn integrated_patch_ids(workdir: &Path) -> HashSet<String> {
    // ponytail: the window is HEAD's last 1000 commits (a branch squashed
    // before that stays listed; widen if it matters).
    let Some(log) = run(
        workdir,
        &["log", "--no-merges", "-p", "-n", "1000", "HEAD"],
        None,
    )
    .ok() else {
        return HashSet::new();
    };
    run(workdir, &["patch-id", "--stable"], Some(&log))
        .ok()
        .map(|out| parse_patch_ids(&String::from_utf8_lossy(&out)))
        .unwrap_or_default()
}

fn is_integrated(workdir: &Path, branch: &str, integrated: &HashSet<String>) -> bool {
    let Some(diff) = run(workdir, &["diff", "--merge-base", "HEAD", branch], None).ok() else {
        return false;
    };
    if diff.is_empty() {
        return true;
    }
    let Some(id) = run(workdir, &["patch-id", "--stable"], Some(&diff))
        .ok()
        .and_then(|out| {
            parse_patch_ids(&String::from_utf8_lossy(&out))
                .into_iter()
                .next()
        })
    else {
        return false;
    };
    integrated.contains(&id)
}

fn parse_patch_ids(out: &str) -> HashSet<String> {
    out.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{parse_contributions, parse_patch_ids};
    use std::collections::HashSet;

    #[test]
    fn parse_contributions_drops_session_heads_and_current() {
        let out = "main\tsubject\ncontrib\tcontrib subject\nfennec/s1\tsession\norigin/fennec/s1\tremote session\norigin/HEAD\tremote head\nfeature\tfeature subject\n";
        let parsed = parse_contributions(out, Some("main"));
        assert_eq!(
            parsed,
            vec![
                ("contrib".to_string(), "contrib subject".to_string()),
                ("feature".to_string(), "feature subject".to_string()),
            ]
        );
    }

    #[test]
    fn parse_contributions_drops_long_lived_and_bot_branches() {
        let out = "main\tmain\norigin/main\tremote main\norigin/develop\tremote develop\nmaster\tmaster\norigin/release-please--branches--main\tbot\nfeat/x\tkeep\norigin/feat/y\tkeep remote\n";
        let parsed = parse_contributions(out, None);
        assert_eq!(
            parsed,
            vec![
                ("feat/x".to_string(), "keep".to_string()),
                ("origin/feat/y".to_string(), "keep remote".to_string()),
            ]
        );
    }

    #[test]
    fn parse_patch_ids_keeps_first_field() {
        let out = "abc123 commit1\n\ndef456 commit2\n";
        assert_eq!(
            parse_patch_ids(out),
            HashSet::from(["abc123".to_string(), "def456".to_string()])
        );
    }
}
