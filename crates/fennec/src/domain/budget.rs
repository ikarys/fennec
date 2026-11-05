//! The human's monthly budget for a project: spending is a decision the
//! human makes; the kernel enforces it, never the model.

use crate::domain::lang::Lang;
use serde::Serialize;

/// The share of the budget past which the human is warned.
pub const ALERT_RATIO: f64 = 0.8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BudgetStatus {
    Under,
    Alert,
    Exceeded,
}

/// The status of a month's spend against the budget.
pub fn status(spent: f64, limit: f64) -> BudgetStatus {
    if spent >= limit {
        BudgetStatus::Exceeded
    } else if spent >= limit * ALERT_RATIO {
        BudgetStatus::Alert
    } else {
        BudgetStatus::Under
    }
}

/// The refusal the kernel records when a turn would start past the budget.
pub fn refusal(lang: Lang, spent: &str, limit: &str) -> String {
    match lang {
        Lang::En => format!(
            "Monthly budget reached for this project ({spent} of {limit}): fennec starts no turn until you raise it on the project page."
        ),
        Lang::Fr => format!(
            "Budget mensuel atteint pour ce projet ({spent} sur {limit}) : fennec ne lance plus de tour tant que vous ne l'augmentez pas sur la page du projet."
        ),
    }
}

/// The alert line, shown on the project page and as a system notification.
pub fn alert(lang: Lang, spent: &str, limit: &str) -> String {
    match lang {
        Lang::En => format!("{spent} of the {limit} monthly budget spent"),
        Lang::Fr => format!("{spent} dépensés sur un budget mensuel de {limit}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_marks_under_alert_and_exceeded() {
        assert_eq!(status(0.0, 100.0), BudgetStatus::Under);
        assert_eq!(status(79.99, 100.0), BudgetStatus::Under);
        assert_eq!(status(80.0, 100.0), BudgetStatus::Alert);
        assert_eq!(status(99.99, 100.0), BudgetStatus::Alert);
        assert_eq!(status(100.0, 100.0), BudgetStatus::Exceeded);
        assert_eq!(status(100.01, 100.0), BudgetStatus::Exceeded);
    }

    #[test]
    fn refusal_and_alert_follow_the_language() {
        assert_eq!(
            refusal(Lang::En, "$80", "$100"),
            "Monthly budget reached for this project ($80 of $100): fennec starts no turn until you raise it on the project page."
        );
        assert_eq!(
            alert(Lang::Fr, "80 $", "100 $"),
            "80 $ dépensés sur un budget mensuel de 100 $"
        );
    }
}
