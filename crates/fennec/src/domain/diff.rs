//! fennec's own (openfox has no integration step): the unified-diff lines an
//! integration adds or removes, parsed from the branch's diff.

/// The lines a diff adds and removes, without their `+`/`-`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChangedLines {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

/// Extract the added and removed lines from a unified diff, which may contain
/// several files.
///
/// Lines before the first `@@` of a file are headers; a `diff --git ` line
/// starts a new file's header again. Inside a hunk, a line starting with `+`
/// is an added line with that one `+` removed, so `++i;` yields `+i;`. A line
/// starting with `-` is a removed line with that one `-` removed, so `--x`
/// yields `-x`; a `--- a/x` header is not a removed line.
pub fn changed_lines(diff: &str) -> ChangedLines {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut in_hunk = false;

    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            in_hunk = false;
            continue;
        }
        if line.starts_with("@@") {
            in_hunk = true;
            continue;
        }
        if in_hunk {
            if let Some(rest) = line.strip_prefix('+') {
                added.push(rest.to_string());
            } else if let Some(rest) = line.strip_prefix('-') {
                removed.push(rest.to_string());
            }
        }
    }

    ChangedLines { added, removed }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_lines_splits_added_and_removed_and_skips_headers() {
        let diff = "+++ b/src/a.rs
@@ -1,2 +1,3 @@
 context
+added one
-removed
+
++kept
";
        let lines = changed_lines(diff);

        assert_eq!(lines.added, vec!["added one", "", "+kept"]);
        assert_eq!(lines.removed, vec!["removed"]);
    }

    #[test]
    fn changed_lines_handles_several_files_in_one_diff() {
        let diff = "diff --git a/x b/x
--- a/x
+++ b/x
@@ -1 +1 @@
+one
diff --git a/y b/y
--- a/y
+++ b/y
@@ -1 +1 @@
+two
";
        let lines = changed_lines(diff);

        assert_eq!(lines.added, vec!["one", "two"]);
        assert!(lines.removed.is_empty());
    }

    #[test]
    fn changed_lines_removed_lines_strip_one_minus() {
        let diff = "diff --git a/x b/x
--- a/x
+++ b/x
@@ -1 +1 @@
--x
";
        let lines = changed_lines(diff);

        assert!(lines.added.is_empty());
        assert_eq!(lines.removed, vec!["-x"]);
    }
}
