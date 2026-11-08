//! The project's monthly budget as a decision the kernel enforces: the
//! human sets the limit, fennec refuses a turn once the month's spend reaches it.

use crate::config::Settings;
use crate::domain::budget::{self, BudgetStatus};
use crate::domain::clock::{month_start_ms, now_ms};
use crate::domain::pricing::{billed_prices, priced};
use crate::store::SqliteStore;
use token_usage::display::usd;
use token_usage::{Filter, GroupBy, Ledger};

/// The project's budget and what it spent this month.
pub struct ProjectBudget {
    pub limit: f64,
    pub spent: f64,
    pub status: BudgetStatus,
    pub priced: bool,
}

/// The project's budget and what it spent this month; `None` when the human set none.
pub fn of_project(
    settings: &Settings,
    store: &SqliteStore,
    project_id: &str,
) -> std::result::Result<Option<ProjectBudget>, String> {
    let Some(limit) = store
        .project_budget(project_id)
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    let ledger = Ledger::open(&settings.usage_path()).map_err(|e| e.to_string())?;
    let filter = Filter {
        product: Some("fennec".into()),
        group_id: Some(project_id.into()),
        since_ms: Some(month_start_ms(now_ms()) as i64),
        ..Default::default()
    };
    let rows = ledger
        .series(&filter, GroupBy::Model)
        .map_err(|e| e.to_string())?;
    let pricing = priced(
        &rows,
        &billed_prices(&store.preferences().map_err(|e| e.to_string())?),
    );
    let spent = pricing.cost.total();
    Ok(Some(ProjectBudget {
        limit,
        spent,
        status: budget::status(spent, limit),
        priced: pricing.priced > 0,
    }))
}

/// The kernel's check before a turn: `Err(refusal text)` once the month's spend reached the budget.
pub fn check(
    settings: &Settings,
    store: &SqliteStore,
    project_id: &str,
) -> std::result::Result<(), String> {
    let budget = match of_project(settings, store, project_id) {
        Ok(budget) => budget,
        Err(e) => {
            // A broken ledger or store must not block the human's turn.
            eprintln!("fennec: budget: {e}");
            return Ok(());
        }
    };
    let Some(budget) = budget else {
        return Ok(());
    };
    if budget.status == BudgetStatus::Exceeded {
        let lang = store
            .preferences()
            .map_err(|e| e.to_string())?
            .display
            .language;
        let spent = usd(budget.spent, lang.locale());
        let limit = usd(budget.limit, lang.locale());
        return Err(budget::refusal(lang, &spent, &limit));
    }
    Ok(())
}
