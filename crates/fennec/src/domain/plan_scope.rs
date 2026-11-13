//! The files a workflow plan says it will touch.
//!
//! The plan is the contract the human approved; a file outside it is a change
//! the human did not see coming.

/// The paths of every `Files:` line of the plan, in order, deduplicated.
pub fn plan_files(plan: &str) -> Vec<String> {
    let mut out = Vec::new();

    for line in plan.lines() {
        // `- Files:`, `* **Files:**`, `**Files**:`: the models drift on the bullet and the bold.
        let label = line.trim().trim_start_matches(['-', '*', ' ']);
        let Some(rest) = label
            .strip_prefix("Files:")
            .or_else(|| label.strip_prefix("Files**:"))
        else {
            continue;
        };
        let rest = rest.trim_start_matches('*').trim();
        if rest.is_empty() {
            continue;
        }

        let items: Vec<String> = if rest.contains('`') {
            let mut items = Vec::new();
            let mut chars = rest.chars();
            while let Some(c) = chars.next() {
                if c == '`' {
                    let mut item = String::new();
                    for ch in chars.by_ref() {
                        if ch == '`' {
                            break;
                        }
                        item.push(ch);
                    }
                    items.push(item);
                }
            }
            items
        } else {
            rest.split(',').map(str::to_string).collect()
        };

        for raw in items {
            let mut item = raw.trim();
            if item.ends_with(" (new)") {
                item = &item[..item.len() - " (new)".len()];
            }
            item = item.trim();
            if item.starts_with("./") {
                item = &item["./".len()..];
            }
            if item.is_empty() {
                continue;
            }
            let item = item.to_string();
            if !out.contains(&item) {
                out.push(item);
            }
        }
    }

    out
}

/// The changed paths not covered by any plan entry, in the changed order.
///
/// A path is covered when it equals a plan entry, when the entry names a
/// directory ending in `/` and the path starts with it, or when the path
/// starts with `<entry>/`. With no plan entries there is nothing to check.
pub fn outside(plan: &[String], changed: &[String]) -> Vec<String> {
    if plan.is_empty() {
        return Vec::new();
    }
    changed
        .iter()
        .filter(|path| !plan.iter().any(|entry| covers(entry, path)))
        .cloned()
        .collect()
}

/// The gate's contract: the plan's entries plus the test files the human
/// validated. An empty plan stays empty (no plan, no check).
pub fn contract(plan: Vec<String>, validated_tests: &[String]) -> Vec<String> {
    if plan.is_empty() {
        return Vec::new();
    }
    let mut out = plan;
    for test in validated_tests {
        let test = test.strip_prefix("./").unwrap_or(test.as_str()).to_string();
        if test.is_empty() {
            continue;
        }
        if !out.contains(&test) {
            out.push(test);
        }
    }
    out
}

fn covers(entry: &str, path: &str) -> bool {
    entry == path
        || (entry.ends_with('/') && path.starts_with(entry))
        || path.starts_with(&format!("{entry}/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_backticked_files_with_new() {
        let plan = "## Plan\n### Step 1\n- Files: `src/a.rs`, `src/b.rs (new)`\n";

        assert_eq!(
            plan_files(plan),
            vec!["src/a.rs".to_string(), "src/b.rs".to_string()]
        );
    }

    #[test]
    fn reads_bold_labels() {
        let plan = "**Files:** `src/a.rs`\n* Files: src/b.rs\n- **Files**: src/c.rs (new)\n";

        assert_eq!(
            plan_files(plan),
            vec![
                "src/a.rs".to_string(),
                "src/b.rs".to_string(),
                "src/c.rs".to_string()
            ]
        );
    }

    #[test]
    fn splits_commas_without_backticks() {
        let plan = "Files: src/a.rs, src/b.rs, ./src/c.rs\n";

        assert_eq!(
            plan_files(plan),
            vec![
                "src/a.rs".to_string(),
                "src/b.rs".to_string(),
                "src/c.rs".to_string()
            ]
        );
    }

    #[test]
    fn directory_entry_covers_children() {
        let plan = plan_files("Files: src/\n");

        assert_eq!(plan, vec!["src/".to_string()]);
        assert_eq!(
            outside(&plan, &["src/a.rs".to_string()]),
            Vec::<String>::new()
        );
        assert_eq!(
            outside(&plan, &["other/a.rs".to_string()]),
            vec!["other/a.rs".to_string()]
        );
    }

    #[test]
    fn ignores_done_when_and_other_lines() {
        let plan = "## Done when\n- [ ] src/a.rs\nGoal: x\n";

        assert_eq!(plan_files(plan), Vec::<String>::new());
    }

    #[test]
    fn empty_plan_gives_nothing_outside() {
        assert_eq!(
            outside(&[], &["src/a.rs".to_string()]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn contract_adds_validated_tests_and_strips_dot_slash() {
        let plan = vec!["src/a.rs".to_string()];
        let validated = vec!["./tests/a.rs".to_string(), "tests/a.rs".to_string()];

        assert_eq!(
            contract(plan, &validated),
            vec!["src/a.rs".to_string(), "tests/a.rs".to_string()]
        );
    }

    #[test]
    fn contract_validated_test_covers_its_path() {
        let plan = vec!["src/a.rs".to_string()];
        let contract = contract(plan, &["tests/a.rs".to_string()]);

        assert_eq!(
            outside(&contract, &["tests/a.rs".to_string()]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn contract_empty_plan_stays_empty() {
        let validated = vec!["tests/a.rs".to_string()];

        assert_eq!(contract(Vec::new(), &validated), Vec::<String>::new());
    }
}
