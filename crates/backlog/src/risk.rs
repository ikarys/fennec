//! Night-mode risks: what a task asks that nobody should run unattended.

use crate::Idea;

const DELETE_TESTS: &[&str] = &[
    "delete the tests",
    "delete tests",
    "delete the test",
    "remove the tests",
    "remove tests",
    "disable the tests",
    "disable tests",
    "skip the tests",
    "skip tests",
    "supprimer les tests",
    "supprime les tests",
    "désactiver les tests",
];
const PUBLISH: &[&str] = &[
    "publish",
    "pypi",
    "crates.io",
    "npm release",
    "deploy",
    "publier",
    "publie ",
    "déploie",
    "déployer",
];
const DELETE_DATA: &[&str] = &[
    "drop table",
    "drop database",
    "truncate table",
    "rm -rf",
    "wipe the database",
    "delete the database",
    "supprimer la base",
];
const PUSH_REMOTE: &[&str] = &[
    "git push",
    "push to main",
    "push to origin",
    "force push",
    "push --force",
    "pousser sur",
];
const CREDENTIALS: &[&str] = &[
    "private key",
    "credentials",
    "secret key",
    "api key",
    "clé privée",
    "clé d'api",
    "identifiants",
];

const PHRASES: &[(Risk, &[&str])] = &[
    (Risk::DeleteTests, DELETE_TESTS),
    (Risk::Publish, PUBLISH),
    (Risk::DeleteData, DELETE_DATA),
    (Risk::PushRemote, PUSH_REMOTE),
    (Risk::Credentials, CREDENTIALS),
];

/// What a task may ask that waits for the human to be there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    DeleteTests,
    Publish,
    DeleteData,
    PushRemote,
    Credentials,
}

impl Risk {
    pub fn label(self) -> &'static str {
        match self {
            Risk::DeleteTests => "delete or disable tests",
            Risk::Publish => "publish or deploy",
            Risk::DeleteData => "delete data",
            Risk::PushRemote => "push to a remote",
            Risk::Credentials => "handle credentials",
        }
    }
}

/// The risks a task's text, plan and sheet name, in `Risk` order, each once.
// ponytail: plain phrase match, so "deploy" also stops a task about deployment
// docs; a false stop costs a restart in the morning, a missed one costs more.
pub fn risks(idea: &Idea) -> Vec<Risk> {
    let mut parts = vec![idea.text.clone()];
    if let Some(plan) = &idea.plan {
        parts.push(plan.clone());
    }
    if let Some(sheet) = &idea.sheet {
        parts.push(sheet.to_markdown(""));
    }

    let normalized = parts
        .join("\n")
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    PHRASES
        .iter()
        .filter(|(_, phrases)| phrases.iter().any(|phrase| normalized.contains(phrase)))
        .map(|(risk, _)| *risk)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idea(text: &str, plan: Option<&str>) -> crate::Idea {
        crate::Idea {
            id: "idea".into(),
            text: text.into(),
            state: crate::IdeaState::Ready,
            session_id: None,
            plan: plan.map(str::to_owned),
            created_at: 0,
            updated_at: 0,
            halted: None,
            started_at: None,
            autonomous: false,
            attachments: Vec::new(),
            sheet: None,
            model_fields: Default::default(),
        }
    }

    #[test]
    fn plain_feature_has_no_risk() {
        let idea = idea("Add a dark theme toggle to the settings page", None);
        assert_eq!(risks(&idea), Vec::new());
    }

    #[test]
    fn delete_tests_and_publish() {
        let idea = idea(
            "Rewrite it in Rust, delete the tests and publish to PyPI",
            None,
        );
        assert_eq!(risks(&idea), vec![Risk::DeleteTests, Risk::Publish]);
    }

    #[test]
    fn normalization_collapses_whitespace_and_case() {
        let idea = idea("Delete   THE\ntests", None);
        assert_eq!(risks(&idea), vec![Risk::DeleteTests]);
    }

    #[test]
    fn risk_in_plan() {
        let idea = idea("Do it", Some("git push to origin"));
        assert_eq!(risks(&idea), vec![Risk::PushRemote]);
    }
}
