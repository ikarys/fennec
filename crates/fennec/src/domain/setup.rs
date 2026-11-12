//! The first-run checklist: what fennec needs before a first useful session, decided from facts the adapter gathers.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckId {
    Model,
    Sandbox,
    Git,
    Project,
    Scanners,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Todo,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub id: CheckId,
    pub status: CheckStatus,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelFact {
    /// No model and no endpoint named yet: the human has a choice to make.
    NotConfigured,
    Reachable {
        model: String,
        listed: bool,
        base_url: String,
    },
    /// `error` names the URL it failed on.
    Unreachable { error: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxFact {
    Ready,
    Missing,
    Blocked(String),
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitFact {
    Ready,
    Missing,
    /// No user.name / user.email: fennec commits the agent's work under the
    /// human's identity, so every commit would fail.
    NoIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    pub model: ModelFact,
    pub sandbox: SandboxFact,
    pub git: GitFact,
    pub projects: usize,
    pub scanners_found: usize,
    pub scanners_known: usize,
}

pub fn checks(facts: &Facts) -> Vec<Check> {
    let model = match &facts.model {
        ModelFact::NotConfigured => Check {
            id: CheckId::Model,
            status: CheckStatus::Todo,
            detail: Some("no model yet: add its endpoint and pick one".to_owned()),
        },
        ModelFact::Reachable {
            model,
            listed,
            base_url,
        } if *listed => Check {
            id: CheckId::Model,
            status: CheckStatus::Ok,
            detail: Some(format!("{model} at {base_url}")),
        },
        ModelFact::Reachable {
            model, base_url, ..
        } => Check {
            id: CheckId::Model,
            status: CheckStatus::Todo,
            detail: Some(if model.is_empty() {
                format!("no model chosen for {base_url}")
            } else {
                format!("{model} is not served by {base_url}")
            }),
        },
        ModelFact::Unreachable { error } => Check {
            id: CheckId::Model,
            status: CheckStatus::Todo,
            detail: Some(error.clone()),
        },
    };

    let sandbox = match &facts.sandbox {
        SandboxFact::Ready => Check {
            id: CheckId::Sandbox,
            status: CheckStatus::Ok,
            detail: None,
        },
        SandboxFact::Missing => Check {
            id: CheckId::Sandbox,
            status: CheckStatus::Todo,
            detail: Some("the sandbox program is not installed (bubblewrap on Linux)".to_owned()),
        },
        SandboxFact::Blocked(detail) => Check {
            id: CheckId::Sandbox,
            status: CheckStatus::Todo,
            detail: Some(detail.clone()),
        },
        SandboxFact::Off => Check {
            id: CheckId::Sandbox,
            status: CheckStatus::Warn,
            detail: Some("turned off in Settings: commands run on the host".to_owned()),
        },
    };

    let git = match facts.git {
        GitFact::Ready => Check {
            id: CheckId::Git,
            status: CheckStatus::Ok,
            detail: None,
        },
        GitFact::Missing => Check {
            id: CheckId::Git,
            status: CheckStatus::Todo,
            detail: Some("git is not on PATH".to_owned()),
        },
        GitFact::NoIdentity => Check {
            id: CheckId::Git,
            status: CheckStatus::Todo,
            detail: Some(
                "git has no identity: run git config --global user.name \"Your Name\" and git config --global user.email you@example.com".to_owned(),
            ),
        },
    };

    let project = if facts.projects == 0 {
        Check {
            id: CheckId::Project,
            status: CheckStatus::Todo,
            detail: Some("no project opened yet".to_owned()),
        }
    } else {
        let detail = if facts.projects == 1 {
            "1 project".to_owned()
        } else {
            format!("{} projects", facts.projects)
        };
        Check {
            id: CheckId::Project,
            status: CheckStatus::Ok,
            detail: Some(detail),
        }
    };

    let scanners = Check {
        id: CheckId::Scanners,
        status: if facts.scanners_found == facts.scanners_known {
            CheckStatus::Ok
        } else {
            CheckStatus::Warn
        },
        detail: Some(format!(
            "{} of {} security scanners found",
            facts.scanners_found, facts.scanners_known
        )),
    };

    vec![model, sandbox, git, project, scanners]
}

pub fn complete(checks: &[Check]) -> bool {
    !checks.iter().any(|check| check.status == CheckStatus::Todo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> Facts {
        Facts {
            model: ModelFact::Reachable {
                model: "default".into(),
                listed: true,
                base_url: "http://localhost".into(),
            },
            sandbox: SandboxFact::Ready,
            git: GitFact::Ready,
            projects: 1,
            scanners_found: 4,
            scanners_known: 4,
        }
    }

    #[test]
    fn model_unreachable_is_todo_with_url() {
        let mut facts = good();
        facts.model = ModelFact::Unreachable {
            error: "http://example/models: timeout".into(),
        };
        let checks = checks(&facts);
        assert_eq!(checks[0].status, CheckStatus::Todo);
        assert_eq!(
            checks[0].detail.as_deref(),
            Some("http://example/models: timeout")
        );
    }

    #[test]
    fn no_model_named_is_todo() {
        let mut facts = good();
        facts.model = ModelFact::NotConfigured;
        let checks = checks(&facts);
        assert_eq!(checks[0].status, CheckStatus::Todo);
        assert!(!complete(&checks));
    }

    #[test]
    fn model_not_listed_is_todo() {
        let mut facts = good();
        facts.model = ModelFact::Reachable {
            model: "gpt".into(),
            listed: false,
            base_url: "http://example".into(),
        };
        let checks = checks(&facts);
        assert_eq!(checks[0].status, CheckStatus::Todo);
        assert_eq!(
            checks[0].detail.as_deref(),
            Some("gpt is not served by http://example")
        );
    }

    #[test]
    fn endpoint_without_a_model_says_none_is_chosen() {
        let mut facts = good();
        facts.model = ModelFact::Reachable {
            model: String::new(),
            listed: false,
            base_url: "http://example".into(),
        };
        let checks = checks(&facts);
        assert_eq!(checks[0].status, CheckStatus::Todo);
        assert_eq!(
            checks[0].detail.as_deref(),
            Some("no model chosen for http://example")
        );
    }

    #[test]
    fn sandbox_off_is_warn_and_complete_true_when_rest_ok() {
        let mut facts = good();
        facts.sandbox = SandboxFact::Off;
        let checks = checks(&facts);
        assert_eq!(checks[1].status, CheckStatus::Warn);
        assert_eq!(
            checks[1].detail.as_deref(),
            Some("turned off in Settings: commands run on the host")
        );
        assert!(complete(&checks));
    }

    #[test]
    fn no_project_is_todo_and_makes_complete_false() {
        let mut facts = good();
        facts.projects = 0;
        let checks = checks(&facts);
        assert_eq!(checks[3].status, CheckStatus::Todo);
        assert_eq!(checks[3].detail.as_deref(), Some("no project opened yet"));
        assert!(!complete(&checks));
    }

    #[test]
    fn git_without_identity_is_todo() {
        let mut facts = good();
        facts.git = GitFact::NoIdentity;
        let checks = checks(&facts);
        assert_eq!(checks[2].status, CheckStatus::Todo);
        assert!(checks[2].detail.as_deref().unwrap().contains("user.email"));
        assert!(!complete(&checks));
    }

    #[test]
    fn scanners_missing_is_warn() {
        let mut facts = good();
        facts.scanners_found = 3;
        facts.scanners_known = 4;
        let checks = checks(&facts);
        assert_eq!(checks[4].status, CheckStatus::Warn);
        assert_eq!(
            checks[4].detail.as_deref(),
            Some("3 of 4 security scanners found")
        );
    }

    #[test]
    fn all_good_is_complete_with_5_checks_in_order() {
        let checks = checks(&good());
        assert_eq!(checks.len(), 5);
        assert_eq!(
            checks.iter().map(|check| check.id).collect::<Vec<_>>(),
            vec![
                CheckId::Model,
                CheckId::Sandbox,
                CheckId::Git,
                CheckId::Project,
                CheckId::Scanners
            ]
        );
        assert!(checks.iter().all(|check| check.status == CheckStatus::Ok));
        assert!(complete(&checks));
    }
}
