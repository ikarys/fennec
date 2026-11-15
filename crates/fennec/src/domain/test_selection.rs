//! Whether a check's test runner ran no test at all: `cargo test <name>` exits 0 when the name matches nothing, which proves nothing.

/// True when the output shows a test runner that selected no test.
pub fn ran_no_test(output: &str) -> bool {
    let counts = libtest_counts(output);
    if !counts.is_empty() {
        return counts.iter().all(|&count| count == 0);
    }

    if pytest_ran_no_test(output) || jest_ran_no_test(output) {
        return true;
    }

    go_ran_nothing(output)
}

fn libtest_counts(output: &str) -> Vec<u64> {
    let mut counts = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("running ") else {
            continue;
        };

        let mut tokens = rest.split_whitespace();
        let Some(number) = tokens.next() else {
            continue;
        };
        let Ok(count) = number.parse::<u64>() else {
            continue;
        };
        let Some(kind) = tokens.next() else {
            continue;
        };

        if (kind == "test" || kind == "tests") && tokens.next().is_none() {
            counts.push(count);
        }
    }

    counts
}

fn pytest_ran_no_test(output: &str) -> bool {
    output.contains("no tests ran") || output.contains("collected 0 items")
}

fn jest_ran_no_test(output: &str) -> bool {
    output.contains("No tests found")
        || output.contains("No test files found")
        || output.contains("No test suite found")
}

fn go_ran_nothing(output: &str) -> bool {
    let ok_lines: Vec<&str> = output
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("ok ") || line.starts_with("ok\t"))
        .collect();

    if ok_lines.is_empty() {
        return output.contains("testing: warning: no tests to run");
    }

    ok_lines
        .iter()
        .all(|line| line.contains("[no tests to run]") || line.contains("[no test files]"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_all_zero_across_two_targets_is_no_test() {
        let output = "     Running unittests src/lib.rs (target/debug/deps/app-1)\n\nrunning 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 12 filtered out; finished in 0.00s\n\n     Running tests/api.rs (target/debug/deps/api-2)\n\nrunning 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n";
        assert!(ran_no_test(output));
    }

    #[test]
    fn cargo_with_one_positive_target_is_not_no_test() {
        let output = "running 1 test\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n\nrunning 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n";
        assert!(!ran_no_test(output));
    }

    #[test]
    fn cargo_compile_error_without_running_line_is_not_no_test() {
        let output = "error[E0425]: cannot find value `x` in this scope\n --> src/lib.rs:1:5\n";
        assert!(!ran_no_test(output));
    }

    #[test]
    fn plain_running_three_tests_is_not_no_test() {
        assert!(!ran_no_test("running 3 tests\n"));
    }

    #[test]
    fn pytest_zero_items_is_no_test() {
        let output = "collected 0 items\nno tests ran in 0.01s\n";
        assert!(ran_no_test(output));
    }

    #[test]
    fn jest_no_tests_found_is_no_test() {
        assert!(ran_no_test("No tests found, exiting with code 1\n"));
    }

    #[test]
    fn vitest_no_test_files_found_is_no_test() {
        assert!(ran_no_test("No test files found, exiting with code 1\n"));
    }

    #[test]
    fn go_ok_line_with_no_tests_to_run_is_no_test() {
        assert!(ran_no_test(
            "ok  \texample.com/pkg\t0.002s [no tests to run]\n"
        ));
    }

    #[test]
    fn go_with_one_ran_package_is_not_no_test() {
        let output = "ok  \texample.com/a\t0.1s\nok  \texample.com/b\t0.002s [no tests to run]\n";
        assert!(!ran_no_test(output));
    }

    #[test]
    fn normal_failing_assertion_output_is_not_no_test() {
        let output = "running 1 test\n\ntest example::test_foo ... FAILED\n\nfailures:\n\n---- example::test_foo stdout ----\nthread 'example::test_foo' panicked at src/lib.rs:1:5:\nassertion `left == right` failed\n";
        assert!(!ran_no_test(output));
    }

    #[test]
    fn empty_output_is_not_no_test() {
        assert!(!ran_no_test(""));
    }
}
