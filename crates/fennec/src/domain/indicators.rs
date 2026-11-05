//! fennec's indicators come only from recorded facts, so they cannot be talked up; every figure here is verified.

/// The commits of a period by authorship.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Authored {
    pub human: usize,
    pub integrated: usize,
    pub ai: usize,
}

/// What the gate recorded of one integration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Integration {
    pub at_ms: u64,
    pub acknowledged: Vec<String>,
}

/// The facts the indicators are measured from.
#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
    pub integrations: Vec<Integration>,
    pub dialogues: usize,
    pub active_waivers: usize,
    pub rule_breaks: usize,
    pub commits: Authored,
    /// This month's spend in dollars; `None` when no model has a price.
    pub month_cost: Option<f64>,
    pub month_start_ms: u64,
    pub map: crate::domain::map_doc::MapFileState,
    pub adrs: usize,
}

/// The counts each indicator is drawn from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub integrations: usize,
    pub month_integrations: usize,
    pub sensitive: usize,
    pub sensitive_unexplained: usize,
    pub security_accepted: usize,
    pub architecture_accepted: usize,
    pub traced: usize,
    pub bypass_accepted: usize,
    pub blockers_accepted: usize,
}

/// Counts the recorded integration facts; one integration is counted once per kind of bypass.
pub fn count(integrations: &[Integration], month_start_ms: u64) -> Counts {
    let mut counts = Counts::default();

    for integration in integrations {
        counts.integrations += 1;
        if integration.at_ms >= month_start_ms {
            counts.month_integrations += 1;
        }

        let acknowledged = &integration.acknowledged;
        let sensitive = acknowledged.iter().any(|kind| kind == "sensitiveTouched");
        if sensitive {
            counts.sensitive += 1;
            if acknowledged
                .iter()
                .any(|kind| kind == "understandingMissing")
            {
                counts.sensitive_unexplained += 1;
            }
        }

        if acknowledged.iter().any(|kind| kind == "securityFindings") {
            counts.security_accepted += 1;
        }
        if acknowledged
            .iter()
            .any(|kind| kind == "architectureViolated")
        {
            counts.architecture_accepted += 1;
        }
        if !acknowledged.iter().any(|kind| kind == "noCriteria") {
            counts.traced += 1;
        }
        if acknowledged
            .iter()
            .any(|kind| kind == "suppressionsAdded" || kind == "testsWeakened")
        {
            counts.bypass_accepted += 1;
        }

        counts.blockers_accepted += acknowledged.len();
    }

    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integration(at_ms: u64, acknowledged: &[&str]) -> Integration {
        Integration {
            at_ms,
            acknowledged: acknowledged
                .iter()
                .map(|kind| (*kind).to_string())
                .collect(),
        }
    }

    #[test]
    fn count_counts_every_recorded_field() {
        let integrations = vec![
            integration(
                10,
                &[
                    "sensitiveTouched",
                    "understandingMissing",
                    "securityFindings",
                    "architectureViolated",
                    "suppressionsAdded",
                    "testsWeakened",
                ],
            ),
            integration(20, &["noCriteria"]),
        ];

        let counts = count(&integrations, 15);

        assert_eq!(counts.integrations, 2);
        assert_eq!(counts.month_integrations, 1);
        assert_eq!(counts.sensitive, 1);
        assert_eq!(counts.sensitive_unexplained, 1);
        assert_eq!(counts.security_accepted, 1);
        assert_eq!(counts.architecture_accepted, 1);
        assert_eq!(counts.traced, 1);
        assert_eq!(counts.bypass_accepted, 1);
        assert_eq!(counts.blockers_accepted, 7);
    }

    #[test]
    fn count_counts_bypass_once_per_integration() {
        let integrations = vec![integration(10, &["suppressionsAdded", "testsWeakened"])];

        let counts = count(&integrations, 0);

        assert_eq!(counts.bypass_accepted, 1);
    }
}
