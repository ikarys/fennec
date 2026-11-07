//! The project's monthly budget: the human sets it, the server answers
//! display-ready figures and the kernel's status.

use crate::web::http::HttpError;
use serde::Serialize;
use serde_json::json;
use std::net::TcpStream;
use token_usage::display::usd;

use crate::budget::ProjectBudget;
use crate::domain::budget::{self, BudgetStatus};
use crate::domain::lang::Lang;
use crate::web::App;
use crate::web::http::{Request, error, send_json};
use crate::web::views::project_dashboard::Tone;

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetView {
    pub limit_usd: Option<f64>,
    pub label: String,
    pub figure: String,
    pub detail: String,
    pub tone: Tone,
    pub active: bool,
}

/// `GET /api/projects/{id}/budget`.
pub fn get(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    if store.project(id)?.is_none() {
        return error(stream, 404, "no such project");
    }
    let budget = crate::budget::of_project(&app.settings, &store, id)?;
    let lang = Lang::from_code(&store.preferences()?.display.language);
    let view = view(budget.as_ref(), lang);
    send_json(stream, 200, &json!(view))
}

/// `POST /api/projects/{id}/budget` with `{"limitUsd": number | null}`.
pub fn set(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    if store.project(id)?.is_none() {
        return error(stream, 404, "no such project");
    }
    let body = req.json()?;
    let limit = match body.get("limitUsd") {
        Some(value) if value.is_null() => None,
        Some(value) => match value.as_f64() {
            Some(limit) if limit.is_finite() && limit > 0.0 => Some(limit),
            _ => return error(stream, 400, "limitUsd must be a finite number > 0"),
        },
        None => return error(stream, 400, "limitUsd is missing"),
    };
    store.set_project_budget(id, limit)?;
    let budget = crate::budget::of_project(&app.settings, &store, id)?;
    let lang = Lang::from_code(&store.preferences()?.display.language);
    let view = view(budget.as_ref(), lang);
    send_json(stream, 200, &json!(view))
}

fn view(budget: Option<&ProjectBudget>, lang: Lang) -> BudgetView {
    let label = lang.pick("Monthly budget", "Budget mensuel").to_string();
    match budget {
        None => BudgetView {
            limit_usd: None,
            label,
            figure: "—".to_string(),
            detail: lang
                .pick("none: click to set one", "aucun : cliquez pour en fixer un")
                .to_string(),
            tone: Tone::Neutral,
            active: false,
        },
        Some(budget) => {
            let figure = format!("{} %", (budget.spent / budget.limit * 100.0).round() as u64);
            let mut detail = budget::alert(
                lang,
                &usd(budget.spent, lang.locale()),
                &usd(budget.limit, lang.locale()),
            );
            if !budget.priced {
                detail.push_str(lang.pick(
                    " (no billed model has a price: Settings, Models)",
                    " (aucun modèle facturé n'a de prix : Paramètres, Modèles)",
                ));
            }
            let tone = match budget.status {
                BudgetStatus::Under => Tone::Ok,
                BudgetStatus::Alert => Tone::Attention,
                BudgetStatus::Exceeded => Tone::Err,
            };
            BudgetView {
                limit_usd: Some(budget.limit),
                label,
                figure,
                detail,
                tone,
                active: true,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget(limit: f64, spent: f64, status: BudgetStatus, priced: bool) -> ProjectBudget {
        ProjectBudget {
            limit,
            spent,
            status,
            priced,
        }
    }

    #[test]
    fn view_without_a_budget_is_neutral_and_inactive() {
        let en = view(None, Lang::En);
        assert_eq!(en.label, "Monthly budget");
        assert_eq!(en.figure, "—");
        assert_eq!(en.detail, "none: click to set one");
        assert_eq!(en.tone, Tone::Neutral);
        assert!(!en.active);

        let fr = view(None, Lang::Fr);
        assert_eq!(fr.label, "Budget mensuel");
        assert_eq!(fr.detail, "aucun : cliquez pour en fixer un");
    }

    #[test]
    fn view_with_a_budget_reports_the_share_and_tone() {
        let under = view(
            Some(&budget(100.0, 10.0, BudgetStatus::Under, true)),
            Lang::En,
        );
        assert_eq!(under.figure, "10 %");
        assert_eq!(under.tone, Tone::Ok);
        assert!(under.active);
        assert_eq!(under.limit_usd, Some(100.0));

        let alert = view(
            Some(&budget(100.0, 80.0, BudgetStatus::Alert, true)),
            Lang::En,
        );
        assert_eq!(alert.figure, "80 %");
        assert_eq!(alert.tone, Tone::Attention);

        let exceeded = view(
            Some(&budget(100.0, 100.0, BudgetStatus::Exceeded, false)),
            Lang::Fr,
        );
        assert_eq!(exceeded.figure, "100 %");
        assert_eq!(exceeded.tone, Tone::Err);
        assert!(
            exceeded
                .detail
                .ends_with(" (aucun modèle facturé n'a de prix : Paramètres, Modèles)")
        );
    }
}
