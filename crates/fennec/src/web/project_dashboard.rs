//! The project dashboard (`GET /api/projects/{id}/dashboard`): the tiles and
//! recent sessions the project page draws, computed here so the UI only draws
//! them. Every string is final French display text.

use std::net::TcpStream;
use std::path::Path;

use serde::Serialize;
use serde_json::to_value;
use token_usage::display::{duration, percent, tokens};
use token_usage::{Filter, GroupBy, Ledger, Summary};

use super::App;
use super::http::{error, send_json};
use super::idea_board::Board;
use super::insight::Period;
use super::pricing::{priced, prices};
use super::worktree_routes;
use crate::domain::clock::now_ms;
use crate::domain::map_doc::MapFileState;
use crate::{adr, doc, module_map};
use backlog::Stage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
/// The UI's `BadgeTone`; no tile is in error yet.
pub enum Tone {
    Accent,
    Ok,
    Neutral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TileIcon {
    Sessions,
    Ideas,
    Tokens,
    Time,
    Cost,
    Disk,
    Map,
    Doc,
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
    disk: Option<(u64, u64)>,
    map_state: MapFileState,
    module_count: usize,
    doc_count: usize,
    adr_count: usize,
}

/// `GET /api/projects/{id}/dashboard`.
pub fn serve(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    let Some(project) = store.project(id)? else {
        return error(stream, 404, "no such project");
    };
    let workdir = Path::new(&project.workdir);

    let facts = gather(app, &store, &project, workdir)?;
    let view = view(id, &facts);
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
    let pricing = priced(&models, &prices(&store.preferences()?));

    let disk = worktree_routes::project_disk(app, store, project)?;

    let map = module_map::load(workdir);
    let map_state = module_map::map_file_state(workdir, &map);
    let module_count = map.modules.len();

    let doc_count = doc::load(workdir).len();
    let adr_count = adr::load(workdir).len();

    Ok(Facts {
        sessions,
        idea_stages,
        usage,
        cost_value: pricing.value(),
        cost_detail: pricing.detail(),
        disk,
        map_state,
        module_count,
        doc_count,
        adr_count,
    })
}

fn view(project_id: &str, facts: &Facts) -> DashboardView {
    DashboardView {
        tiles: tiles(project_id, facts),
        recent: recent(facts),
    }
}

fn tiles(project_id: &str, facts: &Facts) -> Vec<TileView> {
    vec![
        sessions_tile(facts),
        ideas_tile(project_id, facts),
        tokens_tile(&facts.usage),
        time_tile(&facts.usage),
        cost_tile(&facts.cost_value, &facts.cost_detail),
        disk_tile(facts.disk),
        map_tile(project_id, &facts.map_state, facts.module_count),
        doc_tile(project_id, facts.doc_count, facts.adr_count),
    ]
}

fn sessions_tile(facts: &Facts) -> TileView {
    let running = facts.sessions.iter().filter(|s| s.running).count();
    let awaiting = facts.sessions.iter().filter(|s| s.awaiting).count();
    let detail = if running == 0 && awaiting == 0 {
        "aucune en cours".to_string()
    } else {
        format!("{running} en cours · {awaiting} à intégrer")
    };

    TileView {
        id: "sessions".into(),
        icon: TileIcon::Sessions,
        label: "Sessions".into(),
        figure: facts.sessions.len().to_string(),
        detail,
        tone: Tone::Accent,
        active: running > 0 || awaiting > 0,
        href: None,
        title: None,
    }
}

fn ideas_tile(project_id: &str, facts: &Facts) -> TileView {
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
        label: "À décider".into(),
        figure: yours.to_string(),
        detail: format!("{running} en cours · {queue} prêtes · {done} terminées"),
        tone: Tone::Accent,
        active: yours > 0,
        href: Some(format!("#/project/{project_id}/tasks")),
        title: None,
    }
}

fn tokens_tile(usage: &Summary) -> TileView {
    let figure = tokens(usage.prompt_tokens.saturating_add(usage.completion_tokens));
    let cache = percent(usage.cached_tokens, usage.prompt_tokens);

    TileView {
        id: "tokens".into(),
        icon: TileIcon::Tokens,
        label: "Tokens · 30 j".into(),
        figure,
        detail: format!("{cache} % relus du cache"),
        tone: Tone::Neutral,
        active: false,
        href: Some("#/insight".into()),
        title: None,
    }
}

fn time_tile(usage: &Summary) -> TileView {
    TileView {
        id: "time".into(),
        icon: TileIcon::Time,
        label: "Temps modèle · 30 j".into(),
        figure: duration(usage.total_secs),
        detail: format!("{} appels", usage.calls),
        tone: Tone::Neutral,
        active: false,
        href: Some("#/insight".into()),
        title: None,
    }
}

fn cost_tile(value: &str, detail: &str) -> TileView {
    TileView {
        id: "cost".into(),
        icon: TileIcon::Cost,
        label: "Coût estimé · 30 j".into(),
        figure: value.to_string(),
        detail: detail.to_string(),
        tone: Tone::Neutral,
        active: false,
        href: Some("#/insight".into()),
        title: Some(detail.to_string()),
    }
}

fn disk_tile(disk: Option<(u64, u64)>) -> TileView {
    let (figure, detail, tone, active) = match disk {
        Some((total, generated)) => {
            let alert = total >= worktree_routes::ALERT_BYTES;
            (
                bytes(total),
                format!("dont {} de cache de build", bytes(generated)),
                if alert { Tone::Accent } else { Tone::Neutral },
                alert,
            )
        }
        None => (
            "—".to_string(),
            "pas encore mesuré".to_string(),
            Tone::Neutral,
            false,
        ),
    };

    TileView {
        id: "disk".into(),
        icon: TileIcon::Disk,
        label: "Espace disque".into(),
        figure,
        detail,
        tone,
        active,
        href: None,
        title: Some("Mesuré sur le disque, scan de moins de 10 min réutilisé".into()),
    }
}

fn map_tile(project_id: &str, state: &MapFileState, module_count: usize) -> TileView {
    let (detail, tone, active) = match state {
        MapFileState::Current => ("docs/map.md à jour".to_string(), Tone::Ok, false),
        MapFileState::Stale => ("docs/map.md périmé".to_string(), Tone::Accent, true),
        MapFileState::Missing => ("pas de docs/map.md".to_string(), Tone::Accent, true),
    };

    TileView {
        id: "map".into(),
        icon: TileIcon::Map,
        label: "Carte des modules".into(),
        figure: module_count.to_string(),
        detail,
        tone,
        active,
        href: Some(format!("#/project/{project_id}/map")),
        title: None,
    }
}

fn doc_tile(project_id: &str, doc_count: usize, adr_count: usize) -> TileView {
    TileView {
        id: "doc".into(),
        icon: TileIcon::Doc,
        label: "Documentation".into(),
        figure: doc_count.to_string(),
        detail: format!("{adr_count} ADR"),
        tone: Tone::Neutral,
        active: false,
        href: Some(format!("#/project/{project_id}/doc")),
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
        })
        .collect()
}

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
            disk: None,
            map_state: MapFileState::Missing,
            module_count: 0,
            doc_count: 0,
            adr_count: 0,
        }
    }

    #[test]
    fn view_tiles_are_in_order() {
        let view = view("p", &facts());
        let ids: Vec<&str> = view.tiles.iter().map(|tile| tile.id.as_str()).collect();

        assert_eq!(
            ids,
            [
                "sessions", "ideas", "tokens", "time", "cost", "disk", "map", "doc"
            ]
        );
    }

    #[test]
    fn ideas_tile_is_active_only_when_yours_is_positive() {
        let mut facts = facts();
        facts.idea_stages = vec![Stage::Queue];
        assert!(!view("p", &facts).tiles[1].active);

        facts.idea_stages = vec![Stage::Yours];
        assert!(view("p", &facts).tiles[1].active);
    }

    #[test]
    fn disk_tile_not_measured() {
        let view = view("p", &facts());
        let disk = &view.tiles[5];

        assert_eq!(disk.figure, "—");
        assert_eq!(disk.detail, "pas encore mesuré");
        assert_eq!(disk.tone, Tone::Neutral);
        assert!(!disk.active);
    }

    #[test]
    fn map_tile_tone_follows_map_file_state() {
        let mut facts = facts();

        facts.map_state = MapFileState::Current;
        let map = &view("p", &facts).tiles[6];
        assert_eq!(map.tone, Tone::Ok);
        assert!(!map.active);

        facts.map_state = MapFileState::Stale;
        let map = &view("p", &facts).tiles[6];
        assert_eq!(map.tone, Tone::Accent);
        assert!(map.active);

        facts.map_state = MapFileState::Missing;
        let map = &view("p", &facts).tiles[6];
        assert_eq!(map.tone, Tone::Accent);
        assert!(map.active);
    }
}
