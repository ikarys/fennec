//! The project dashboard (`GET /api/projects/{id}/dashboard`): the tiles and
//! recent sessions the project page draws, computed here so the UI only draws
//! them. Every string is final French display text.

use crate::web::http::{HttpError, require_project};
use std::net::TcpStream;
use std::path::Path;

use serde::Serialize;
use serde_json::to_value;
use token_usage::display::{duration, percent, tokens};
use token_usage::{Filter, GroupBy, Ledger, Summary};

use crate::adr;
use crate::domain::clock::now_ms;
use crate::domain::lang::Lang;
use crate::domain::pricing::{priced, prices};
use crate::web::App;
use crate::web::http::send_json;
use crate::web::views::idea_board::Board;
use crate::web::views::insight::Period;
use backlog::Stage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
/// The UI's `BadgeTone`; a budget tile can be in error.
pub enum Tone {
    Accent,
    Info,
    Ok,
    Attention,
    Neutral,
    Err,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TileIcon {
    Sessions,
    Ideas,
    Tokens,
    Time,
    Cost,
    Decisions,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TileView {
    pub id: String,
    pub icon: TileIcon,
    pub label: String,
    pub figure: String,
    pub detail: String,
    pub tone: Tone,
    pub active: bool,
    pub href: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentView {
    pub id: String,
    pub title: String,
    pub updated_at: u64,
    pub running: bool,
    pub awaiting: bool,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardView {
    pub tiles: Vec<TileView>,
    pub recent: Vec<RecentView>,
}

struct SessionFact {
    id: String,
    title: String,
    updated_at: u64,
    running: bool,
    awaiting: bool,
}

struct Facts {
    sessions: Vec<SessionFact>,
    idea_stages: Vec<Stage>,
    usage: Summary,
    cost_value: String,
    cost_detail: String,
    adr_count: usize,
    adr_accepted: usize,
    lang: Lang,
}

/// `GET /api/projects/{id}/dashboard`.
pub fn serve(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let project = require_project(&store, id)?;
    let workdir = Path::new(&project.workdir);

    let facts = gather(app, &store, &project, workdir)?;
    let view = view(id, &facts, facts.lang);
    send_json(stream, 200, &to_value(view).map_err(|e| e.to_string())?)
}

fn gather(
    app: &App,
    store: &crate::store::SqliteStore,
    project: &crate::domain::project::Project,
    workdir: &Path,
) -> Result<Facts, String> {
    let sessions = store.sessions(Some(&project.id))?;
    // The log says a workflow ended; git says whether its work still waits.
    let unintegrated = crate::git::sessions_with_unintegrated_work(workdir);

    let sessions = sessions
        .into_iter()
        .filter(|session| session.parent_id.is_none())
        .map(|session| {
            let running = store
                .session_tree(&session.id)?
                .iter()
                .any(|session_id| app.turns.is_running(session_id));
            let awaiting = session.awaiting_integration && unintegrated.contains(&session.id);
            Ok(SessionFact {
                id: session.id,
                title: session.title,
                updated_at: session.updated_at,
                running,
                awaiting,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let backlog = store.backlog(&project.id)?;
    let board = Board::new(app, store, &backlog, project);
    let idea_stages = board.stages()?;

    let filter = Filter {
        product: Some("fennec".into()),
        group_id: Some(project.id.clone()),
        since_ms: Period::Days(30).since_ms(now_ms()),
        ..Default::default()
    };

    let ledger = Ledger::open(&app.settings.usage_path()).map_err(|e| e.to_string())?;
    let usage = ledger.summary(&filter).map_err(|e| e.to_string())?;
    let models = ledger
        .series(&filter, GroupBy::Model)
        .map_err(|e| e.to_string())?;
    // The dashboard's cost text follows the human's display language.
    let preferences = store.preferences()?;
    let lang = preferences.display.language;
    let pricing = priced(&models, &prices(&preferences));

    let adrs = adr::load(workdir);
    let adr_accepted = adrs
        .iter()
        .filter(|adr| adr.status.as_deref() == Some("accepted"))
        .count();

    Ok(Facts {
        sessions,
        idea_stages,
        usage,
        cost_value: pricing.value(lang.locale()),
        cost_detail: pricing.detail(lang),
        adr_count: adrs.len(),
        adr_accepted,
        lang,
    })
}

fn view(project_id: &str, facts: &Facts, lang: Lang) -> DashboardView {
    DashboardView {
        tiles: tiles(project_id, facts, lang),
        recent: recent(facts),
    }
}

fn tiles(project_id: &str, facts: &Facts, lang: Lang) -> Vec<TileView> {
    vec![
        sessions_tile(project_id, facts, lang),
        ideas_tile(project_id, facts, lang),
        tokens_tile(&facts.usage, lang),
        time_tile(&facts.usage, lang),
        cost_tile(
            &facts.cost_value,
            &facts.cost_detail,
            facts.usage.calls > 0,
            lang,
        ),
        decisions_tile(project_id, facts.adr_count, facts.adr_accepted, lang),
    ]
}

fn sessions_tile(project_id: &str, facts: &Facts, lang: Lang) -> TileView {
    let running = facts.sessions.iter().filter(|s| s.running).count();
    let awaiting = facts.sessions.iter().filter(|s| s.awaiting).count();
    let detail = if running == 0 && awaiting == 0 {
        lang.pick("none running", "aucune en cours").to_string()
    } else {
        match lang {
            Lang::En => format!("{running} running · {awaiting} to integrate"),
            Lang::Fr => format!("{running} en cours · {awaiting} à intégrer"),
        }
    };

    TileView {
        id: "sessions".into(),
        icon: TileIcon::Sessions,
        label: lang.pick("Sessions", "Sessions").into(),
        figure: facts.sessions.len().to_string(),
        detail,
        tone: Tone::Accent,
        active: running > 0 || awaiting > 0,
        // The project page has a Sessions tab, so the tile deep-links to it.
        href: Some(format!("#/project/{project_id}/sessions")),
        title: None,
    }
}

fn ideas_tile(project_id: &str, facts: &Facts, lang: Lang) -> TileView {
    let count = |stage: Stage| facts.idea_stages.iter().filter(|s| **s == stage).count();
    let (yours, running, queue, done) = (
        count(Stage::Yours),
        count(Stage::Running),
        count(Stage::Queue),
        count(Stage::Done),
    );

    TileView {
        id: "ideas".into(),
        icon: TileIcon::Ideas,
        label: lang.pick("To decide", "À décider").into(),
        figure: yours.to_string(),
        detail: match lang {
            Lang::En => format!("{running} running · {queue} ready · {done} done"),
            Lang::Fr => format!("{running} en cours · {queue} prêtes · {done} terminées"),
        },
        tone: Tone::Accent,
        active: yours > 0,
        href: Some(format!("#/project/{project_id}/tasks")),
        title: None,
    }
}

fn tokens_tile(usage: &Summary, lang: Lang) -> TileView {
    let figure = tokens(
        usage.prompt_tokens.saturating_add(usage.completion_tokens),
        lang.locale(),
    );
    let cache = percent(usage.cached_tokens, usage.prompt_tokens);

    TileView {
        id: "tokens".into(),
        icon: TileIcon::Tokens,
        label: lang.pick("Tokens · 30 d", "Tokens · 30 j").into(),
        figure,
        detail: match lang {
            Lang::En => format!("{cache} % re-read from cache"),
            Lang::Fr => format!("{cache} % relus du cache"),
        },
        // Colour the tile only when the ledger has calls; an empty window stays neutral.
        tone: if usage.calls > 0 {
            Tone::Ok
        } else {
            Tone::Neutral
        },
        active: usage.calls > 0,
        href: Some("#/insight".into()),
        title: None,
    }
}

fn time_tile(usage: &Summary, lang: Lang) -> TileView {
    TileView {
        id: "time".into(),
        icon: TileIcon::Time,
        label: lang.pick("Model time · 30 d", "Temps modèle · 30 j").into(),
        figure: duration(usage.total_secs, lang.locale()),
        detail: match lang {
            Lang::En => format!("{} calls", usage.calls),
            Lang::Fr => format!("{} appels", usage.calls),
        },
        // Model time is informational when present; no calls means no figure to highlight.
        tone: if usage.calls > 0 {
            Tone::Info
        } else {
            Tone::Neutral
        },
        active: usage.calls > 0,
        href: Some("#/insight".into()),
        title: None,
    }
}

fn cost_tile(value: &str, detail: &str, active: bool, lang: Lang) -> TileView {
    // Cost is the most decision-relevant usage figure, so it gets attention when present.
    TileView {
        id: "cost".into(),
        icon: TileIcon::Cost,
        label: lang
            .pick("Estimated cost · 30 d", "Coût estimé · 30 j")
            .into(),
        figure: value.to_string(),
        detail: detail.to_string(),
        tone: if active {
            Tone::Attention
        } else {
            Tone::Neutral
        },
        active,
        href: Some("#/insight".into()),
        title: Some(detail.to_string()),
    }
}

fn decisions_tile(project_id: &str, adr_count: usize, accepted: usize, lang: Lang) -> TileView {
    TileView {
        id: "decisions".into(),
        icon: TileIcon::Decisions,
        label: lang.pick("Decisions", "Décisions").into(),
        figure: adr_count.to_string(),
        detail: match lang {
            Lang::Fr => format!("{accepted} acceptée(s)"),
            Lang::En => format!("{accepted} accepted"),
        },
        tone: Tone::Neutral,
        active: false,
        href: Some(format!("#/project/{project_id}/decisions")),
        title: None,
    }
}

fn recent(facts: &Facts) -> Vec<RecentView> {
    let mut sessions: Vec<&SessionFact> = facts.sessions.iter().collect();
    sessions.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    sessions.truncate(5);

    sessions
        .into_iter()
        .map(|session| RecentView {
            id: session.id.clone(),
            title: session.title.clone(),
            updated_at: session.updated_at,
            running: session.running,
            awaiting: session.awaiting,
        })
        .collect()
}

#[cfg(test)]
fn bytes(value: u64) -> String {
    if value < 1024 {
        return format!("{value} o");
    }

    let mut size = value as f64 / 1024.0;
    for unit in ["Ko", "Mo", "Go"] {
        if size < 1024.0 {
            return format_size(size, unit);
        }
        size /= 1024.0;
    }
    format_size(size, "To")
}

#[cfg(test)]
fn format_size(size: f64, unit: &str) -> String {
    if size < 10.0 {
        format!("{size:.1} {unit}").replace('.', ",")
    } else {
        format!("{} {unit}", size.round() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_formats_bytes() {
        assert_eq!(bytes(512), "512 o");
        assert_eq!(bytes(2_791_728_742), "2,6 Go");
        assert_eq!(bytes(15 * 1024 * 1024 * 1024), "15 Go");
    }

    fn summary() -> Summary {
        Summary {
            calls: 0,
            prompt_tokens: 0,
            cached_tokens: 0,
            completion_tokens: 0,
            cache_hit_rate: 0.0,
            lost_cache_calls: 0,
            lost_cache_tokens: 0,
            total_secs: 0.0,
            breakdown: Default::default(),
            savings: Default::default(),
        }
    }

    fn facts() -> Facts {
        Facts {
            sessions: Vec::new(),
            idea_stages: Vec::new(),
            usage: summary(),
            cost_value: "—".into(),
            cost_detail: "aucun modèle n'a de prix : Paramètres, Modèles".into(),
            adr_count: 0,
            adr_accepted: 0,
            lang: Lang::Fr,
        }
    }

    #[test]
    fn view_tiles_are_in_order() {
        let view = view("p", &facts(), Lang::Fr);
        let ids: Vec<&str> = view.tiles.iter().map(|tile| tile.id.as_str()).collect();

        assert_eq!(
            ids,
            ["sessions", "ideas", "tokens", "time", "cost", "decisions"]
        );
    }

    #[test]
    fn ideas_tile_is_active_only_when_yours_is_positive() {
        let mut facts = facts();
        facts.idea_stages = vec![Stage::Queue];
        assert!(!view("p", &facts, Lang::Fr).tiles[1].active);

        facts.idea_stages = vec![Stage::Yours];
        assert!(view("p", &facts, Lang::Fr).tiles[1].active);
    }

    #[test]
    fn tiles_are_english_when_the_language_is_english() {
        let view = view("p", &facts(), Lang::En);
        assert_eq!(view.tiles[1].label, "To decide");
        assert_eq!(view.tiles[2].label, "Tokens · 30 d");
    }
}
