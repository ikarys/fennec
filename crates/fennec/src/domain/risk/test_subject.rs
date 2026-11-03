//! Which source files should have a test of its own.

/// Whether the untested signal applies: false for a file that needs no test
/// of its own (a declaration-only module, test support, build tooling).
pub fn needs_own_test(path: &str, content: &str) -> bool {
    !(is_build_tooling(path) || is_test_support(path) || is_declaration_only(path, content))
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn file_stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(0) => name,
        Some(pos) => &name[..pos],
        None => name,
    }
}

// Build files are configuration or tooling, not product code under test.
fn is_build_tooling(path: &str) -> bool {
    let name = file_name(path);
    name == "build.rs"
        || name.contains(".config.")
        || name.ends_with(".d.ts")
        || path.split('/').any(|segment| segment == "scripts")
}

// Test support files exist to serve tests, so they do not need their own test.
fn is_test_support(path: &str) -> bool {
    let name = file_name(path);
    let stem = file_stem(name);
    let stem_is_support = matches!(
        stem,
        "test_support"
            | "test_utils"
            | "test_helpers"
            | "testing"
            | "fixtures"
            | "conftest"
            | "setupTests"
    ) || stem.ends_with(".setup");
    let dir_is_support = path
        .split('/')
        .any(|segment| matches!(segment, "fixtures" | "__mocks__" | "testdata"));
    stem_is_support || dir_is_support
}

fn is_declaration_only(path: &str, content: &str) -> bool {
    let name = file_name(path);
    if name.ends_with(".rs") {
        is_rust_declaration(content)
    } else if name.ends_with(".ts")
        || name.ends_with(".tsx")
        || name.ends_with(".js")
        || name.ends_with(".jsx")
        || name.ends_with(".mjs")
    {
        is_ts_declaration(content)
    } else {
        false
    }
}

// Comments can hide statements, so remove them before classifying a file.
// ponytail: string literals are not parsed, a `//` or `/*` inside one cuts the
// rest; a real tokenizer if it ever misclassifies a file.
fn strip_comments(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '/' {
            match chars.peek() {
                Some(&'/') => {
                    chars.next();
                    for ch in chars.by_ref() {
                        if ch == '\n' {
                            out.push(ch);
                            break;
                        }
                    }
                }
                Some(&'*') => {
                    chars.next();
                    out.push(' ');
                    while let Some(ch) = chars.next() {
                        if ch == '*' && chars.peek() == Some(&'/') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => out.push(c),
            }
        } else {
            out.push(c);
        }
    }

    out
}

// Split into candidate statements; TS/JS may separate statements with newlines.
fn statements(content: &str, split_newlines: bool) -> impl Iterator<Item = &str> {
    content
        .split(move |c| c == ';' || (split_newlines && c == '\n'))
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn is_rust_declaration(content: &str) -> bool {
    let stripped = strip_comments(content);
    statements(&stripped, false).all(is_rust_declaration_piece)
}

fn is_rust_declaration_piece(piece: &str) -> bool {
    let mut rest = piece.trim();

    while rest.starts_with("#[") || rest.starts_with("#![") {
        match rest.find(']') {
            Some(end) => rest = rest[end + 1..].trim(),
            None => return false,
        }
    }

    if let Some(stripped) = strip_visibility(rest) {
        rest = stripped;
    }

    rest.starts_with("mod ") || rest.starts_with("use ") || rest.starts_with("extern crate ")
}

// Visibility is optional on `mod` and `use`, so remove it before checking the keyword.
fn strip_visibility(rest: &str) -> Option<&str> {
    if !rest.starts_with("pub") {
        return None;
    }

    let after = &rest[3..];
    let next = after.chars().next()?;
    if next != '(' && !next.is_whitespace() {
        return None;
    }

    let after = after.trim_start();
    if after.starts_with('(') {
        after.find(')').map(|end| after[end + 1..].trim())
    } else {
        Some(after)
    }
}

fn is_ts_declaration(content: &str) -> bool {
    let stripped = strip_comments(content);
    statements(&stripped, true).all(|piece| {
        piece.starts_with("import ")
            || piece.starts_with("export * ")
            || piece.starts_with("export {")
            || piece.starts_with("export type {")
            || piece.starts_with("export type * ")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_tooling_needs_no_test() {
        assert!(!needs_own_test("crates/fennec/build.rs", ""));
        assert!(!needs_own_test("web/vite.config.ts", ""));
        assert!(!needs_own_test("web/src/vite-env.d.ts", ""));
        assert!(!needs_own_test("web/scripts/check-ui.mjs", ""));
    }

    #[test]
    fn test_support_needs_no_test() {
        assert!(!needs_own_test("src/web/test_support.rs", ""));
        assert!(!needs_own_test("web/src/vitest.setup.ts", ""));
        assert!(!needs_own_test("src/fixtures/data.rs", ""));
        assert!(!needs_own_test("web/src/__mocks__/api.ts", ""));
        assert!(!needs_own_test("src/testdata/file.rs", ""));
    }

    #[test]
    fn declaration_only_needs_no_test() {
        let domain = concat!(
            "//! The vocabulary.\n\n",
            "pub mod event;\npub mod stats;\n",
            "#[cfg(feature = \"x\")]\npub mod extra;\n",
        );
        assert!(!needs_own_test("src/domain.rs", domain));

        let lib = "pub use crate::model::{\n    a,\n    b,\n};\n";
        assert!(!needs_own_test("src/lib.rs", lib));

        let index = "export * from './a'\nexport { b } from './b'\n";
        assert!(!needs_own_test("web/src/lib/index.ts", index));

        assert!(!needs_own_test("src/empty.rs", "// nothing\n"));
    }

    #[test]
    fn non_declaration_files_need_test() {
        assert!(needs_own_test("src/turn.rs", "pub fn turn() {}"));
        assert!(needs_own_test(
            "src/model.rs",
            "#[derive(Debug)] struct Foo { a: u8 }"
        ));
        assert!(needs_own_test(
            "web/src/lib/fold.ts",
            "export function fold() {}"
        ));
        assert!(needs_own_test(
            "src/main.rs",
            "use std::io;\n\nfn main() {}"
        ));
    }
}
