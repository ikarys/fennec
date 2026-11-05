//! The security scanners fennec runs when it finds them on PATH (the integration gate, the audit-security agent); fennec shows which are installed and how to install the others, it never installs one.

use crate::domain::lang::Lang;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scanner {
    pub id: &'static str,
    pub name: &'static str,
    /// The executable looked up on PATH.
    pub binary: &'static str,
    /// What it checks, for the human, in the human's language.
    pub covers_en: &'static str,
    pub covers_fr: &'static str,
    /// The command the human runs to install it.
    pub install: &'static str,
    pub url: &'static str,
}

impl Scanner {
    pub fn covers(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::En => self.covers_en,
            Lang::Fr => self.covers_fr,
        }
    }
}

pub const SCANNERS: [Scanner; 4] = [
    Scanner {
        id: "cargo-audit",
        name: "cargo audit",
        binary: "cargo-audit",
        covers_en: "Rust dependencies with known vulnerabilities (RustSec).",
        covers_fr: "Dépendances Rust aux vulnérabilités connues (RustSec).",
        install: "cargo install cargo-audit --locked && cargo audit",
        url: "https://github.com/rustsec/rustsec/tree/main/cargo-audit",
    },
    Scanner {
        id: "osv-scanner",
        name: "OSV-Scanner",
        binary: "osv-scanner",
        covers_en: "Dependencies from any ecosystem (npm, PyPI, Go, crates…) against the OSV database.",
        covers_fr: "Dépendances de tout écosystème (npm, PyPI, Go, crates…) contre la base OSV.",
        install: "go install github.com/google/osv-scanner/v2/cmd/osv-scanner@latest",
        url: "https://github.com/google/osv-scanner",
    },
    Scanner {
        id: "gitleaks",
        name: "Gitleaks",
        binary: "gitleaks",
        covers_en: "Committed secrets, including git history.",
        covers_fr: "Secrets commités, historique git compris.",
        install: "go install github.com/zricethezav/gitleaks/v8@latest",
        url: "https://github.com/gitleaks/gitleaks",
    },
    Scanner {
        id: "semgrep",
        name: "Semgrep",
        binary: "semgrep",
        covers_en: "Dangerous code patterns: injections, weak crypto, deserialization…",
        covers_fr: "Motifs de code dangereux : injections, crypto faible, désérialisation…",
        install: "pipx install semgrep",
        url: "https://semgrep.dev/docs/getting-started/",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanner_ids_are_unique_and_fields_are_non_empty() {
        let mut ids = Vec::new();
        for scanner in SCANNERS {
            assert!(!scanner.id.is_empty());
            assert!(!scanner.name.is_empty());
            assert!(!scanner.binary.is_empty());
            assert!(!scanner.covers_en.is_empty());
            assert!(!scanner.covers_fr.is_empty());
            assert!(!scanner.install.is_empty());
            assert!(!scanner.url.is_empty());
            ids.push(scanner.id);
        }
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), SCANNERS.len());
    }
}
