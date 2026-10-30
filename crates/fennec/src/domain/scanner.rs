//! The security scanners the audit-security agent runs when it finds them on PATH; fennec shows which are installed and how to install the others, it never installs one.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scanner {
    pub id: &'static str,
    pub name: &'static str,
    /// The executable looked up on PATH.
    pub binary: &'static str,
    /// What it checks, for the human (French, shown as is).
    pub covers: &'static str,
    /// The command the human runs to install it.
    pub install: &'static str,
    pub url: &'static str,
}

pub const SCANNERS: [Scanner; 4] = [
    Scanner {
        id: "cargo-audit",
        name: "cargo audit",
        binary: "cargo-audit",
        covers: "Dépendances Rust aux vulnérabilités connues (RustSec).",
        install: "cargo install cargo-audit --locked && cargo audit",
        url: "https://github.com/rustsec/rustsec/tree/main/cargo-audit",
    },
    Scanner {
        id: "osv-scanner",
        name: "OSV-Scanner",
        binary: "osv-scanner",
        covers: "Dépendances de tout écosystème (npm, PyPI, Go, crates…) contre la base OSV.",
        install: "go install github.com/google/osv-scanner/v2/cmd/osv-scanner@latest",
        url: "https://github.com/google/osv-scanner",
    },
    Scanner {
        id: "gitleaks",
        name: "Gitleaks",
        binary: "gitleaks",
        covers: "Secrets commités, historique git compris.",
        install: "go install github.com/zricethezav/gitleaks/v8@latest",
        url: "https://github.com/gitleaks/gitleaks",
    },
    Scanner {
        id: "semgrep",
        name: "Semgrep",
        binary: "semgrep",
        covers: "Motifs de code dangereux : injections, crypto faible, désérialisation…",
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
            assert!(!scanner.covers.is_empty());
            assert!(!scanner.install.is_empty());
            assert!(!scanner.url.is_empty());
            ids.push(scanner.id);
        }
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), SCANNERS.len());
    }
}
