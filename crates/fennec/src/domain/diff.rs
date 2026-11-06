//! The unified-diff lines an integration adds or removes, parsed from the
//! branch's diff; one file's diff as numbered hunks, for the Changes page.

use serde::Serialize;

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

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct FileDiff {
    pub binary: bool,
    pub truncated: bool,
    pub hunks: Vec<Hunk>,
}

/// Past this many lines the rest of a file's diff is dropped (`truncated`): a
/// regenerated lockfile would otherwise ship megabytes to the page.
pub const MAX_DIFF_LINES: usize = 5000;

/// One file's unified diff as numbered lines, hunk by hunk. Lines before the
/// first `@@` are headers; a malformed `@@` skips the lines up to the next one.
pub fn file_diff(diff: &str) -> FileDiff {
    let mut out = FileDiff::default();
    let (mut old, mut new) = (0u32, 0u32);
    let mut pushed = 0;
    for line in diff.lines() {
        if line.starts_with("@@") {
            match parse_hunk_header(line) {
                Some((start_old, start_new, header)) => {
                    (old, new) = (start_old, start_new);
                    out.hunks.push(Hunk {
                        header,
                        lines: Vec::new(),
                    });
                }
                None => (old, new) = (0, 0),
            }
            continue;
        }
        let Some(hunk) = out.hunks.last_mut().filter(|_| old > 0 || new > 0) else {
            out.binary |= line.starts_with("Binary files ");
            continue;
        };
        let (kind, text) = match line.split_at_checked(1) {
            Some((" ", text)) => (DiffLineKind::Context, text),
            Some(("-", text)) => (DiffLineKind::Removed, text),
            Some(("+", text)) => (DiffLineKind::Added, text),
            // `\ No newline at end of file`, or a header of a malformed hunk.
            Some(_) => continue,
            // Some tools strip a blank context line's trailing space.
            None => (DiffLineKind::Context, ""),
        };
        if pushed == MAX_DIFF_LINES {
            out.truncated = true;
            break;
        }
        let old_number = (kind != DiffLineKind::Added).then(|| {
            old += 1;
            old - 1
        });
        let new_number = (kind != DiffLineKind::Removed).then(|| {
            new += 1;
            new - 1
        });
        hunk.lines.push(DiffLine {
            kind,
            old: old_number,
            new: new_number,
            text: text.to_string(),
        });
        pushed += 1;
    }
    out
}

fn parse_hunk_header(line: &str) -> Option<(u32, u32, String)> {
    let rest = line.strip_prefix("@@")?;
    let (left, section) = rest.split_once("@@")?;
    let mut parts = left.split_whitespace();
    let old = parts
        .next()?
        .strip_prefix('-')?
        .split(',')
        .next()?
        .parse::<u32>()
        .ok()?;
    let new = parts
        .next()?
        .strip_prefix('+')?
        .split(',')
        .next()?
        .parse::<u32>()
        .ok()?;
    Some((old, new, section.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_diff_parses_two_hunks_with_numbering_and_headers() {
        let diff = "\
diff --git a/src/a.rs b/src/a.rs
@@ -1,2 +1,2 @@ fn main() {
 x
-y
+z
@@ -10,2 +10,2 @@
 a
-b
+c
";
        let d = file_diff(diff);
        assert!(!d.binary);
        assert!(!d.truncated);
        assert_eq!(d.hunks.len(), 2);
        assert_eq!(d.hunks[0].header, "fn main() {");
        assert_eq!(
            d.hunks[0].lines,
            vec![
                DiffLine {
                    kind: DiffLineKind::Context,
                    old: Some(1),
                    new: Some(1),
                    text: "x".into()
                },
                DiffLine {
                    kind: DiffLineKind::Removed,
                    old: Some(2),
                    new: None,
                    text: "y".into()
                },
                DiffLine {
                    kind: DiffLineKind::Added,
                    old: None,
                    new: Some(2),
                    text: "z".into()
                },
            ]
        );
        assert_eq!(d.hunks[1].header, "");
        assert_eq!(
            d.hunks[1].lines,
            vec![
                DiffLine {
                    kind: DiffLineKind::Context,
                    old: Some(10),
                    new: Some(10),
                    text: "a".into()
                },
                DiffLine {
                    kind: DiffLineKind::Removed,
                    old: Some(11),
                    new: None,
                    text: "b".into()
                },
                DiffLine {
                    kind: DiffLineKind::Added,
                    old: None,
                    new: Some(11),
                    text: "c".into()
                },
            ]
        );
    }

    #[test]
    fn file_diff_marks_binary_files() {
        let diff = "Binary files a/lock and b/lock differ\n";
        let d = file_diff(diff);
        assert!(d.binary);
        assert!(d.hunks.is_empty());
    }

    #[test]
    fn file_diff_ignores_no_newline_markers() {
        let diff =
            "@@ -1 +1 @@\n-x\n\\ No newline at end of file\n+y\n\\ No newline at end of file\n";
        let d = file_diff(diff);
        assert_eq!(d.hunks.len(), 1);
        assert_eq!(d.hunks[0].lines.len(), 2);
        assert_eq!(d.hunks[0].lines[0].text, "x");
        assert_eq!(d.hunks[0].lines[1].text, "y");
    }

    #[test]
    fn file_diff_truncates_long_output() {
        let mut diff = String::from("@@ -1 +1 @@\n");
        for i in 0..MAX_DIFF_LINES + 10 {
            diff.push_str(&format!("+line{}\n", i));
        }
        let d = file_diff(&diff);
        assert!(d.truncated);
        assert_eq!(d.hunks.len(), 1);
        assert_eq!(d.hunks[0].lines.len(), MAX_DIFF_LINES);
        assert_eq!(d.hunks[0].lines[0].new, Some(1));
        assert_eq!(
            d.hunks[0].lines[MAX_DIFF_LINES - 1].new,
            Some(MAX_DIFF_LINES as u32)
        );
    }

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
