//! The project's indicators: read-only figures drawn only from recorded facts.

use crate::web::http::{HttpError, require_project};
use std::collections::BTreeMap;
use std::net::TcpStream;
use std::path::Path;

use serde_json::json;
use token_usage::{Filter, GroupBy, Ledger};

use crate::config::Settings;
use crate::domain::clock::{month_start_ms, now_ms};
use crate::domain::drift;
use crate::domain::event::Event;
use crate::domain::indicators::{Facts, Integration};
use crate::domain::lang::Lang;
use crate::domain::pricing::{priced, prices};
use crate::domain::waiver;
use crate::store::SqliteStore;
use crate::web::App;
use crate::web::http::send_json;
use crate::web::views::indicators;
use crate::{adr, architecture, module_map, risk};

pub fn serve(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let project = require_project(&store, id)?;
    let lang = Lang::from_code(&store.preferences()?.display.language);
    let facts = facts(app, &store, &project.id, Path::new(&project.workdir))?;
    send_json(
        stream,
        200,
        &json!({ "indicators": indicators::view(&facts, lang) }),
    )
}

/// The recorded facts the indicators are drawn from: the project's log, its repository, the ledger.
fn facts(
    app: &App,
    store: &SqliteStore,
    project_id: &str,
    workdir: &Path,
) -> Result<Facts, String> {
    let now = now_ms();
    let events = store.project_events(
        project_id,
        &[
            "integration.applied",
            "understanding.started",
            "waiver.requested",
            "waiver.answered",
        ],
    )?;

    let mut integrations = Vec::new();
    let mut dialogues = 0;
    let mut waivers: BTreeMap<String, Vec<Event>> = BTreeMap::new();

    for event in events {
        match event.event {
            Event::IntegrationApplied { acknowledged, .. } => integrations.push(Integration {
                at_ms: event.at_ms,
                acknowledged,
            }),
            Event::UnderstandingStarted { .. } => dialogues += 1,
            Event::WaiverRequested { .. } | Event::WaiverAnswered { .. } => {
                waivers
                    .entry(event.session_id)
                    .or_default()
                    .push(event.event);
            }
            _ => {}
        }
    }

    let active_waivers = waivers
        .values()
        .map(|session_events| waiver::active(session_events, now).len())
        .sum();

    let mut map = module_map::load(workdir);
    let rule_breaks = drift::measure(
        &mut map,
        &architecture::load(workdir),
        module_map::committed_map_file(workdir).as_deref(),
    )
    .violations
    .len();
    let map_state = module_map::map_file_state(workdir, &map);
    let adrs = adr::load(workdir).len();
    let commits = risk::recent_authorship(workdir, 30);
    let month_start = month_start_ms(now);

    let month_cost = match month_cost(&app.settings, store, project_id, now) {
        Ok(cost) => cost,
        Err(e) => {
            // A broken ledger must not hide the other verified indicators.
            eprintln!("fennec: indicators: {e}");
            None
        }
    };

    Ok(Facts {
        integrations,
        dialogues,
        active_waivers,
        rule_breaks,
        commits,
        month_cost,
        month_start_ms: month_start,
        map: map_state,
        adrs,
    })
}

/// The display-ready indicators, in the fixed order the project page shows them.
/// This month's spend in dollars; `None` when no model has a price.
fn month_cost(
    settings: &Settings,
    store: &SqliteStore,
    project_id: &str,
    now: u64,
) -> std::result::Result<Option<f64>, String> {
    let ledger = Ledger::open(&settings.usage_path()).map_err(|e| e.to_string())?;
    let filter = Filter {
        product: Some("fennec".into()),
        group_id: Some(project_id.into()),
        since_ms: Some(month_start_ms(now) as i64),
        ..Default::default()
    };
    let rows = ledger
        .series(&filter, GroupBy::Model)
        .map_err(|e| e.to_string())?;
    let pricing = priced(
        &rows,
        &prices(&store.preferences().map_err(|e| e.to_string())?),
    );

    if pricing.priced == 0 {
        Ok(None)
    } else {
        Ok(Some(pricing.cost.total()))
    }
}
