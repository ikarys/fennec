//! The project's indicators: read-only figures drawn only from recorded facts.

use crate::web::http::HttpError;
use std::collections::BTreeMap;
use std::net::TcpStream;
use std::path::Path;

use serde::Serialize;
use serde_json::json;
use token_usage::display::usd;
use token_usage::{Filter, GroupBy, Ledger};

use crate::config::Settings;
use crate::domain::clock::{month_start_ms, now_ms};
use crate::domain::drift;
use crate::domain::event::Event;
use crate::domain::indicators::{self, Facts, Integration};
use crate::domain::lang::Lang;
use crate::domain::map_doc::MapFileState;
use crate::domain::pricing::{priced, prices};
use crate::domain::waiver;
use crate::store::SqliteStore;
use crate::web::App;
use crate::web::http::{error, send_json};
use crate::web::views::project_dashboard::Tone;
use crate::{adr, architecture, module_map, risk};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IndicatorId {
    Integrity,
    SensitiveReview,
    Understanding,
    Architecture,
    Security,
    Cost,
    Transmission,
    Traceability,
    ReviewLoad,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndicatorView {
    pub id: IndicatorId,
    pub label: String,
    pub figure: String,
    pub detail: String,
    pub tone: Tone,
}

/// `GET /api/projects/{id}/indicators`.
pub fn serve(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let Some(project) = store.project(id)? else {
        return error(stream, 404, "no such project");
    };
    let lang = Lang::from_code(&store.preferences()?.display.language);
    let facts = facts(app, &store, &project.id, Path::new(&project.workdir))?;
    send_json(stream, 200, &json!({ "indicators": view(&facts, lang) }))
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
fn view(facts: &Facts, lang: Lang) -> Vec<IndicatorView> {
    let counts = indicators::count(&facts.integrations, facts.month_start_ms);
    vec![
        integrity(facts, lang),
        sensitive_review(&counts, lang),
        understanding(facts, lang),
        architecture(facts, &counts, lang),
        security(&counts, lang),
        cost(facts, &counts, lang),
        transmission(facts, lang),
        traceability(&counts, lang),
        review_load(&counts, lang),
    ]
}

fn integrity(facts: &Facts, lang: Lang) -> IndicatorView {
    let label = match lang {
        Lang::En => "Integrity".to_string(),
        Lang::Fr => "Intégrité".to_string(),
    };
    let commits = facts.commits;

    if commits.human == 0 && commits.integrated == 0 && commits.ai == 0 {
        let detail = match lang {
            Lang::En => "no commit in 30 days".to_string(),
            Lang::Fr => "aucun commit en 30 jours".to_string(),
        };
        return IndicatorView {
            id: IndicatorId::Integrity,
            label,
            figure: "—".to_string(),
            detail,
            tone: Tone::Neutral,
        };
    }

    let detail = match lang {
        Lang::En => format!(
            "{} AI commits outside fennec, {} through fennec, {} by hand (30 days)",
            commits.ai, commits.integrated, commits.human
        ),
        Lang::Fr => format!(
            "{} commits IA hors fennec, {} par fennec, {} à la main (30 jours)",
            commits.ai, commits.integrated, commits.human
        ),
    };
    let tone = if commits.ai == 0 {
        Tone::Ok
    } else {
        Tone::Attention
    };

    IndicatorView {
        id: IndicatorId::Integrity,
        label,
        figure: commits.ai.to_string(),
        detail,
        tone,
    }
}

fn sensitive_review(counts: &indicators::Counts, lang: Lang) -> IndicatorView {
    let label = match lang {
        Lang::En => "Sensitive review".to_string(),
        Lang::Fr => "Revue des zones sensibles".to_string(),
    };

    if counts.sensitive == 0 {
        let detail = match lang {
            Lang::En => "no integration touched a sensitive zone".to_string(),
            Lang::Fr => "aucune intégration n'a touché de zone sensible".to_string(),
        };
        return IndicatorView {
            id: IndicatorId::SensitiveReview,
            label,
            figure: "—".to_string(),
            detail,
            tone: Tone::Neutral,
        };
    }

    let reviewed = counts.sensitive - counts.sensitive_unexplained;
    let percent = (reviewed as f64 / counts.sensitive as f64 * 100.0).round() as u64;
    let detail = match lang {
        Lang::En => format!(
            "{reviewed} of {} sensitive integrations had a comprehension dialogue",
            counts.sensitive
        ),
        Lang::Fr => format!(
            "{reviewed} sur {} intégrations sensibles avec un dialogue de compréhension",
            counts.sensitive
        ),
    };
    let tone = if counts.sensitive_unexplained == 0 {
        Tone::Ok
    } else {
        Tone::Err
    };

    IndicatorView {
        id: IndicatorId::SensitiveReview,
        label,
        figure: format!("{percent} %"),
        detail,
        tone,
    }
}

fn understanding(facts: &Facts, lang: Lang) -> IndicatorView {
    let label = match lang {
        Lang::En => "Comprehension".to_string(),
        Lang::Fr => "Compréhension".to_string(),
    };
    let detail = match lang {
        Lang::En => "comprehension dialogues opened on a branch".to_string(),
        Lang::Fr => "dialogues de compréhension ouverts sur une branche".to_string(),
    };
    let tone = if facts.dialogues > 0 {
        Tone::Info
    } else {
        Tone::Neutral
    };

    IndicatorView {
        id: IndicatorId::Understanding,
        label,
        figure: facts.dialogues.to_string(),
        detail,
        tone,
    }
}

fn architecture(facts: &Facts, counts: &indicators::Counts, lang: Lang) -> IndicatorView {
    let label = match lang {
        Lang::En => "Architecture".to_string(),
        Lang::Fr => "Architecture".to_string(),
    };
    let detail = match lang {
        Lang::En => format!(
            "{} module rule breaks now, {} violations accepted at integration, {} active waivers",
            facts.rule_breaks, counts.architecture_accepted, facts.active_waivers
        ),
        Lang::Fr => format!(
            "{} règles de modules enfreintes, {} violations acceptées à l'intégration, {} dérogations actives",
            facts.rule_breaks, counts.architecture_accepted, facts.active_waivers
        ),
    };
    let tone = if facts.rule_breaks > 0 {
        Tone::Err
    } else if counts.architecture_accepted + facts.active_waivers > 0 {
        Tone::Attention
    } else {
        Tone::Ok
    };

    IndicatorView {
        id: IndicatorId::Architecture,
        label,
        figure: facts.rule_breaks.to_string(),
        detail,
        tone,
    }
}

fn security(counts: &indicators::Counts, lang: Lang) -> IndicatorView {
    let label = match lang {
        Lang::En => "Security".to_string(),
        Lang::Fr => "Sécurité".to_string(),
    };

    if counts.integrations == 0 {
        let detail = match lang {
            Lang::En => "no integration yet".to_string(),
            Lang::Fr => "aucune intégration pour l'instant".to_string(),
        };
        return IndicatorView {
            id: IndicatorId::Security,
            label,
            figure: "—".to_string(),
            detail,
            tone: Tone::Neutral,
        };
    }

    let detail = match lang {
        Lang::En => format!(
            "{} of {} integrations accepted scanner findings",
            counts.security_accepted, counts.integrations
        ),
        Lang::Fr => format!(
            "{} sur {} intégrations ont accepté des alertes de scanner",
            counts.security_accepted, counts.integrations
        ),
    };
    let tone = if counts.security_accepted == 0 {
        Tone::Ok
    } else {
        Tone::Attention
    };

    IndicatorView {
        id: IndicatorId::Security,
        label,
        figure: counts.security_accepted.to_string(),
        detail,
        tone,
    }
}

fn cost(facts: &Facts, counts: &indicators::Counts, lang: Lang) -> IndicatorView {
    let label = match lang {
        Lang::En => "Cost per integration".to_string(),
        Lang::Fr => "Coût par intégration".to_string(),
    };

    let Some(cost) = facts.month_cost else {
        let detail = match lang {
            Lang::En => "no model has a price".to_string(),
            Lang::Fr => "aucun modèle n'a de prix".to_string(),
        };
        return IndicatorView {
            id: IndicatorId::Cost,
            label,
            figure: "—".to_string(),
            detail,
            tone: Tone::Neutral,
        };
    };

    if counts.month_integrations == 0 {
        let spent = usd(cost, lang.locale());
        let detail = match lang {
            Lang::En => format!("{spent} this month, no integration yet"),
            Lang::Fr => format!("{spent} ce mois-ci, aucune intégration"),
        };
        return IndicatorView {
            id: IndicatorId::Cost,
            label,
            figure: "—".to_string(),
            detail,
            tone: Tone::Neutral,
        };
    }

    let spent = usd(cost, lang.locale());
    let per_integration = usd(cost / counts.month_integrations as f64, lang.locale());
    let detail = match lang {
        Lang::En => format!(
            "{spent} this month for {} integrations",
            counts.month_integrations
        ),
        Lang::Fr => format!(
            "{spent} ce mois-ci pour {} intégrations",
            counts.month_integrations
        ),
    };

    IndicatorView {
        id: IndicatorId::Cost,
        label,
        figure: per_integration,
        detail,
        tone: Tone::Info,
    }
}

fn transmission(facts: &Facts, lang: Lang) -> IndicatorView {
    let label = match lang {
        Lang::En => "Transmission".to_string(),
        Lang::Fr => "Transmission".to_string(),
    };
    let (figure, tone) = match facts.map {
        MapFileState::Current => ("current", Tone::Ok),
        MapFileState::Stale => ("stale", Tone::Attention),
        MapFileState::Missing => ("missing", Tone::Neutral),
    };
    let figure = match lang {
        Lang::En => figure.to_string(),
        Lang::Fr => match facts.map {
            MapFileState::Current => "à jour".to_string(),
            MapFileState::Stale => "périmée".to_string(),
            MapFileState::Missing => "absente".to_string(),
        },
    };
    let detail = match lang {
        Lang::En => format!("docs/map.md, {} ADRs", facts.adrs),
        Lang::Fr => format!("docs/map.md, {} ADR", facts.adrs),
    };

    IndicatorView {
        id: IndicatorId::Transmission,
        label,
        figure,
        detail,
        tone,
    }
}

fn traceability(counts: &indicators::Counts, lang: Lang) -> IndicatorView {
    let label = match lang {
        Lang::En => "Traceability".to_string(),
        Lang::Fr => "Traçabilité".to_string(),
    };

    if counts.integrations == 0 {
        let detail = match lang {
            Lang::En => "no integration yet".to_string(),
            Lang::Fr => "aucune intégration pour l'instant".to_string(),
        };
        return IndicatorView {
            id: IndicatorId::Traceability,
            label,
            figure: "—".to_string(),
            detail,
            tone: Tone::Neutral,
        };
    }

    let percent = (counts.traced as f64 / counts.integrations as f64 * 100.0).round() as u64;
    let detail = match lang {
        Lang::En => format!(
            "{} of {} integrations tied to an approved plan",
            counts.traced, counts.integrations
        ),
        Lang::Fr => format!(
            "{} sur {} intégrations reliées à un plan approuvé",
            counts.traced, counts.integrations
        ),
    };
    let tone = if counts.traced == counts.integrations {
        Tone::Ok
    } else {
        Tone::Attention
    };

    IndicatorView {
        id: IndicatorId::Traceability,
        label,
        figure: format!("{percent} %"),
        detail,
        tone,
    }
}

fn review_load(counts: &indicators::Counts, lang: Lang) -> IndicatorView {
    let label = match lang {
        Lang::En => "Review load".to_string(),
        Lang::Fr => "Charge de revue".to_string(),
    };

    if counts.integrations == 0 {
        let detail = match lang {
            Lang::En => "no integration yet".to_string(),
            Lang::Fr => "aucune intégration pour l'instant".to_string(),
        };
        return IndicatorView {
            id: IndicatorId::ReviewLoad,
            label,
            figure: "—".to_string(),
            detail,
            tone: Tone::Neutral,
        };
    }

    let figure = format!(
        "{:.1}",
        counts.blockers_accepted as f64 / counts.integrations as f64
    );
    let figure = match lang {
        Lang::En => figure,
        Lang::Fr => figure.replace('.', ","),
    };
    let detail = match lang {
        Lang::En => format!(
            "blockers accepted per integration; {} accepted a bypass (a silenced check, weakened tests)",
            counts.bypass_accepted
        ),
        Lang::Fr => format!(
            "blocages acceptés par intégration ; {} ont accepté un contournement (contrôle réduit au silence, tests affaiblis)",
            counts.bypass_accepted
        ),
    };
    let tone = if counts.bypass_accepted > 0 {
        Tone::Attention
    } else {
        Tone::Ok
    };

    IndicatorView {
        id: IndicatorId::ReviewLoad,
        label,
        figure,
        detail,
        tone,
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::indicators::Authored;

    fn facts(integrations: Vec<Integration>, dialogues: usize, month_cost: Option<f64>) -> Facts {
        Facts {
            integrations,
            dialogues,
            active_waivers: 0,
            rule_breaks: 0,
            commits: Authored::default(),
            month_cost,
            month_start_ms: 0,
            map: MapFileState::Current,
            adrs: 0,
        }
    }

    fn by_id(views: &[IndicatorView], id: IndicatorId) -> &IndicatorView {
        views
            .iter()
            .find(|view| view.id == id)
            .unwrap_or_else(|| panic!("missing indicator {id:?}"))
    }

    #[test]
    fn view_without_integrations_is_neutral_for_integration_indicators() {
        let views = view(&facts(Vec::new(), 0, Some(10.0)), Lang::En);

        let security = by_id(&views, IndicatorId::Security);
        assert_eq!(security.figure, "—");
        assert_eq!(security.detail, "no integration yet");
        assert_eq!(security.tone, Tone::Neutral);

        let traceability = by_id(&views, IndicatorId::Traceability);
        assert_eq!(traceability.figure, "—");
        assert_eq!(traceability.tone, Tone::Neutral);

        let review_load = by_id(&views, IndicatorId::ReviewLoad);
        assert_eq!(review_load.figure, "—");
        assert_eq!(review_load.tone, Tone::Neutral);
    }

    #[test]
    fn sensitive_integration_without_dialogue_is_err() {
        let integrations = vec![Integration {
            at_ms: 1,
            acknowledged: vec![
                "sensitiveTouched".to_string(),
                "understandingMissing".to_string(),
            ],
        }];
        let views = view(&facts(integrations, 0, None), Lang::En);
        let sensitive = by_id(&views, IndicatorId::SensitiveReview);

        assert_eq!(sensitive.figure, "0 %");
        assert_eq!(sensitive.tone, Tone::Err);
    }

    #[test]
    fn view_labels_follow_french() {
        let views = view(&facts(Vec::new(), 0, None), Lang::Fr);

        assert_eq!(by_id(&views, IndicatorId::Integrity).label, "Intégrité");
        assert_eq!(
            by_id(&views, IndicatorId::SensitiveReview).label,
            "Revue des zones sensibles"
        );
        assert_eq!(
            by_id(&views, IndicatorId::Understanding).label,
            "Compréhension"
        );
        assert_eq!(by_id(&views, IndicatorId::Security).label, "Sécurité");
        assert_eq!(
            by_id(&views, IndicatorId::Cost).label,
            "Coût par intégration"
        );
        assert_eq!(
            by_id(&views, IndicatorId::Traceability).label,
            "Traçabilité"
        );
        assert_eq!(
            by_id(&views, IndicatorId::ReviewLoad).label,
            "Charge de revue"
        );
    }

    #[test]
    fn cost_figure_is_the_month_cost_per_integration() {
        let integrations = vec![Integration {
            at_ms: 1,
            acknowledged: Vec::new(),
        }];
        let views = view(&facts(integrations, 0, Some(100.0)), Lang::En);
        let cost = by_id(&views, IndicatorId::Cost);

        assert_eq!(cost.figure, usd(100.0, Lang::En.locale()));
        assert_eq!(cost.tone, Tone::Info);
    }
}
