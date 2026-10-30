//! The Insight page, computed here so the UI only draws it (`GET /api/insight`):
//! the usage ledger over a period, for every project or one. The ledger
//! outlives the sessions, so a deleted session still counts.

use serde::Serialize;
use serde_json::json;
use std::net::TcpStream;
use token_usage::display::{duration, percent, tokens, usd};
use token_usage::{Filter, GroupBy, Ledger, Row, Summary};

use super::App;
use super::context_view::{CompositionView, composition_view};
use super::http::{Request, error, send_json};
use crate::domain::clock::now_ms;
use crate::domain::context_parts::PartKind;
use crate::domain::tariff::Tariff;
use std::collections::BTreeMap;

const DAY_MS: u64 = 86_400_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Period {
    Days(u64),
    All,
}

impl Period {
    /// `7`, `30`, `90` or `all`; anything else is refused. Absent means 30 days.
    pub fn parse(raw: Option<&str>) -> Option<Period> {
        match raw {
            None => Some(Period::Days(30)),
            Some("7") => Some(Period::Days(7)),
            Some("30") => Some(Period::Days(30)),
            Some("90") => Some(Period::Days(90)),
            Some("all") => Some(Period::All),
            Some(_) => None,
        }
    }

    /// The first instant counted, none for `All`.
    pub fn since_ms(self, now_ms: u64) -> Option<i64> {
        match self {
            Period::Days(days) => Some(now_ms.saturating_sub(days.saturating_mul(DAY_MS)) as i64),
            Period::All => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightView {
    /// "Tous les projets", or the project's name.
    pub scope: String,
    pub cards: Vec<CardView>,
    /// One bar per day with calls, oldest first.
    pub days: Vec<DayView>,
    /// Per project, busiest first; empty when the view is one project's.
    pub projects: Vec<RowView>,
    /// Per model, busiest first.
    pub models: Vec<RowView>,
    /// What the prompts sent were made of, summed over the calls; none before
    /// any call was broken down. Indicative, like the context gauge's.
    pub consumers: Option<CompositionView>,
    /// Every project the ledger knows, by name, for the scope picker.
    pub project_options: Vec<ProjectOption>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardView {
    pub label: String,
    pub value: String,
    pub detail: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayView {
    pub day: String,
    pub label: String,
    /// Heights against the busiest day's input, 0 to 100, stacked: cached,
    /// then fresh (re-read neither from cache nor lost), then lost.
    pub cached_percent: u64,
    pub fresh_percent: u64,
    pub lost_percent: u64,
    pub title: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowView {
    pub key: String,
    pub name: String,
    pub calls: String,
    pub input: String,
    pub cache: String,
    pub output: String,
    pub lost: String,
    pub cost: String,
    pub time: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectOption {
    pub id: String,
    pub name: String,
}

/// What the ledger answered for one view.
pub struct InsightData {
    pub total: Summary,
    pub days: Vec<Row>,
    pub projects: Vec<Row>,
    pub models: Vec<Row>,
    pub groups: Vec<Row>,
}

impl InsightData {
    pub fn view(self, project: Option<&str>, tariff: &Tariff) -> InsightView {
        let InsightData {
            total,
            days,
            projects,
            models,
            groups,
        } = self;

        let scope = match project {
            Some(id) => groups
                .iter()
                .find(|row| row.key == id)
                .map(|row| row.name.clone())
                .unwrap_or_else(|| id.to_string()),
            None => "Tous les projets".to_string(),
        };

        let cards = cards(&total, tariff);
        let consumers = consumers(&total.breakdown);

        let max = days
            .iter()
            .map(|row| row.summary.prompt_tokens)
            .max()
            .unwrap_or(0)
            .max(1);
        let days = days.iter().map(|row| day_view(row, max)).collect();

        let projects = if project.is_none() {
            let mut rows = projects;
            busiest_first(&mut rows);
            rows.into_iter()
                .map(|row| row_view(&row, false, tariff))
                .collect()
        } else {
            Vec::new()
        };

        let mut models = models;
        busiest_first(&mut models);
        let models = models
            .into_iter()
            .map(|row| row_view(&row, true, tariff))
            .collect();

        let project_options = project_options(&groups);

        InsightView {
            scope,
            cards,
            days,
            projects,
            models,
            consumers,
            project_options,
        }
    }
}

/// `GET /api/insight?project=<id>&days=<7|30|90|all>`.
pub fn serve(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let period = match Period::parse(req.query_param("days").as_deref()) {
        Some(period) => period,
        None => return error(stream, 400, "days must be 7, 30, 90 or all"),
    };

    let project = req.query_param("project").filter(|s| !s.is_empty());

    let ledger = Ledger::open(&app.settings.usage_path()).map_err(|e| e.to_string())?;

    let base = Filter {
        product: Some("fennec".to_string()),
        group_id: project.clone(),
        since_ms: period.since_ms(now_ms()),
        ..Default::default()
    };

    let total = ledger.summary(&base).map_err(|e| e.to_string())?;
    let days = ledger
        .series(&base, GroupBy::Day)
        .map_err(|e| e.to_string())?;
    let projects = if project.is_none() {
        ledger
            .series(&base, GroupBy::Group)
            .map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };
    let models = ledger
        .series(&base, GroupBy::Model)
        .map_err(|e| e.to_string())?;

    // The picker needs every project, not only the selected period.
    let groups = ledger
        .series(
            &Filter {
                product: Some("fennec".to_string()),
                ..Default::default()
            },
            GroupBy::Group,
        )
        .map_err(|e| e.to_string())?;

    let data = InsightData {
        total,
        days,
        projects,
        models,
        groups,
    };

    let tariff = app.store()?.preferences()?.tariff;

    send_json(stream, 200, &json!(data.view(project.as_deref(), &tariff)))
}

fn busiest_first(rows: &mut [Row]) {
    rows.sort_by(|a, b| {
        b.summary
            .prompt_tokens
            .cmp(&a.summary.prompt_tokens)
            .then_with(|| a.key.cmp(&b.key))
    });
}

fn cost(summary: &Summary, tariff: &Tariff) -> f64 {
    tariff.cost(
        summary.prompt_tokens,
        summary.cached_tokens,
        summary.completion_tokens,
    )
}

fn cards(total: &Summary, tariff: &Tariff) -> Vec<CardView> {
    let fresh = total.prompt_tokens.saturating_sub(total.cached_tokens);
    let cache = percent(total.cached_tokens, total.prompt_tokens);
    let lost_detail = if total.lost_cache_calls == 0 {
        "aucun appel n'a relu son préfixe".to_string()
    } else {
        format!(
            "{} appels ont relu ce que le serveur avait déjà vu",
            total.lost_cache_calls
        )
    };

    vec![
        CardView {
            label: "Appels".into(),
            value: total.calls.to_string(),
            detail: format!("temps modèle {}", duration(total.total_secs)),
        },
        CardView {
            label: "Entrée".into(),
            value: tokens(total.prompt_tokens),
            detail: format!("{} hors cache", tokens(fresh)),
        },
        CardView {
            label: "Cache".into(),
            value: format!("{cache} %"),
            detail: format!("{} lus du cache", tokens(total.cached_tokens)),
        },
        CardView {
            label: "Cache perdu".into(),
            value: tokens(total.lost_cache_tokens),
            detail: lost_detail,
        },
        CardView {
            label: "Sortie".into(),
            value: tokens(total.completion_tokens),
            detail: "tokens générés".into(),
        },
        CardView {
            label: "Coût estimé".into(),
            value: usd(cost(total, tariff)),
            detail: format!(
                "indicatif, au tarif {} / {} / {} par M (entrée, cache, sortie)",
                usd(tariff.input_per_m),
                usd(tariff.cached_per_m),
                usd(tariff.output_per_m)
            ),
        },
    ]
}

/// The ledger's `<kind>/<label>` keys, largest first; keys of another
/// shape (an older or foreign writer) are left out.
fn consumers(breakdown: &BTreeMap<String, u64>) -> Option<CompositionView> {
    let mut shares: Vec<(PartKind, String, u64)> = breakdown
        .iter()
        .filter_map(|(key, &tokens)| {
            let (kind, label) = PartKind::split_key(key)?;
            Some((kind, label.to_string(), tokens))
        })
        .filter(|(_, _, tokens)| *tokens > 0)
        .collect();
    if shares.is_empty() {
        return None;
    }
    shares.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.1.cmp(&b.1)));
    let total = shares.iter().map(|(_, _, tokens)| tokens).sum();
    Some(composition_view(&shares, total, 0))
}

fn day_view(row: &Row, max: u64) -> DayView {
    let prompt = row.summary.prompt_tokens;
    let cached = row.summary.cached_tokens;
    let lost = row
        .summary
        .lost_cache_tokens
        .min(prompt.saturating_sub(cached));
    let fresh = prompt.saturating_sub(cached).saturating_sub(lost);

    let label = day_label(&row.key);
    let calls = row.summary.calls;
    let input = tokens(prompt);
    let cache = percent(cached, prompt);
    let lost_text = tokens(lost);
    let output = tokens(row.summary.completion_tokens);

    let title = format!(
        "{label} · {calls} appels · entrée {input} (cache {cache} %) · cache perdu {lost_text} · sortie {output}"
    );

    DayView {
        day: row.key.clone(),
        label,
        cached_percent: percent(cached, max),
        fresh_percent: percent(fresh, max),
        lost_percent: percent(lost, max),
        title,
    }
}

fn row_view(row: &Row, model: bool, tariff: &Tariff) -> RowView {
    let name = if model {
        row.key.clone()
    } else {
        row.name.clone()
    };
    let cache = percent(row.summary.cached_tokens, row.summary.prompt_tokens);
    let lost = if row.summary.lost_cache_tokens == 0 {
        "—".to_string()
    } else {
        tokens(row.summary.lost_cache_tokens)
    };

    RowView {
        key: row.key.clone(),
        name,
        calls: row.summary.calls.to_string(),
        input: tokens(row.summary.prompt_tokens),
        cache: format!("{cache} %"),
        output: tokens(row.summary.completion_tokens),
        lost,
        cost: usd(cost(&row.summary, tariff)),
        time: duration(row.summary.total_secs),
    }
}

fn project_options(groups: &[Row]) -> Vec<ProjectOption> {
    let mut options: Vec<ProjectOption> = groups
        .iter()
        .map(|row| ProjectOption {
            id: row.key.clone(),
            name: row.name.clone(),
        })
        .collect();

    options.sort_by_cached_key(|option| (option.name.to_lowercase(), option.id.clone()));
    options
}

fn day_label(key: &str) -> String {
    let mut parts = key.split('-');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(_year), Some(month), Some(day)) => format!("{day}/{month}"),
        _ => key.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(
        calls: u64,
        prompt: u64,
        cached: u64,
        completion: u64,
        lost_calls: u64,
        lost_tokens: u64,
        secs: f64,
    ) -> Summary {
        Summary {
            calls,
            prompt_tokens: prompt,
            cached_tokens: cached,
            completion_tokens: completion,
            cache_hit_rate: 0.0,
            lost_cache_calls: lost_calls,
            lost_cache_tokens: lost_tokens,
            total_secs: secs,
            breakdown: Default::default(),
            savings: Default::default(),
        }
    }

    fn row(key: &str, name: &str, summary: Summary) -> Row {
        Row {
            key: key.to_string(),
            name: name.to_string(),
            summary,
        }
    }

    #[test]
    fn period_parse_and_since() {
        assert_eq!(Period::parse(None), Some(Period::Days(30)));
        assert_eq!(Period::parse(Some("7")), Some(Period::Days(7)));
        assert_eq!(Period::parse(Some("all")), Some(Period::All));
        assert_eq!(Period::parse(Some("12")), None);

        assert_eq!(
            Period::Days(1).since_ms(DAY_MS * 3),
            Some(DAY_MS as i64 * 2)
        );
        assert_eq!(Period::All.since_ms(DAY_MS * 3), None);
    }

    #[test]
    fn view_without_project() {
        let total = summary(10, 1000, 970, 100, 0, 0, 1.0);
        let days = vec![
            row(
                "2026-10-28",
                "2026-10-28",
                summary(5, 2000, 1000, 200, 10, 100, 2.0),
            ),
            row(
                "2026-10-29",
                "2026-10-29",
                summary(1, 1000, 500, 50, 0, 0, 1.0),
            ),
        ];
        let projects = vec![
            row("p1", "Projet un", summary(1, 1000, 0, 10, 0, 0, 1.0)),
            row("p2", "Projet deux", summary(9, 3000, 0, 90, 0, 0, 2.0)),
        ];
        let models = vec![row("m1", "m1", summary(10, 4000, 0, 100, 0, 0, 3.0))];
        let groups = vec![row("p1", "Projet un", summary(1, 1000, 0, 10, 0, 0, 1.0))];

        let view = InsightData {
            total,
            days,
            projects,
            models,
            groups,
        }
        .view(None, &Tariff::default());

        assert_eq!(view.scope, "Tous les projets");

        let labels: Vec<&str> = view.cards.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                "Appels",
                "Entrée",
                "Cache",
                "Cache perdu",
                "Sortie",
                "Coût estimé"
            ]
        );
        assert_eq!(view.cards[2].value, "97 %");

        assert_eq!(view.days[0].label, "28/10");
        assert_eq!(
            view.days[0].cached_percent + view.days[0].fresh_percent + view.days[0].lost_percent,
            100
        );
        assert_eq!(view.days[1].cached_percent, 25);
        assert_eq!(view.days[1].fresh_percent, 25);
        assert_eq!(view.days[1].lost_percent, 0);

        assert_eq!(view.projects[0].key, "p2");
        assert_eq!(view.projects[0].lost, "—");
        assert!(!view.projects[0].cost.is_empty());
    }

    #[test]
    fn view_with_project() {
        let total = summary(1, 100, 0, 10, 0, 0, 1.0);
        let groups = vec![row("p1", "Projet un", summary(1, 100, 0, 10, 0, 0, 1.0))];

        let view = InsightData {
            total,
            days: vec![],
            projects: vec![],
            models: vec![],
            groups,
        }
        .view(Some("p1"), &Tariff::default());

        assert_eq!(view.scope, "Projet un");
        assert!(view.projects.is_empty());
    }

    #[test]
    fn consumers_group_the_breakdown_by_kind_largest_first() {
        let breakdown: BTreeMap<String, u64> = [
            ("prompt/AGENTS.md".to_string(), 300),
            ("tools/Résultat : read_file".to_string(), 600),
            ("conversation/Messages humains".to_string(), 100),
            ("legacy".to_string(), 999),
        ]
        .into();
        let view = consumers(&breakdown).expect("consumers");

        let labels: Vec<&str> = view.parts.iter().map(|p| p.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Résultat : read_file", "AGENTS.md", "Messages humains"]
        );
        assert_eq!(view.parts[0].percent, 60);
        assert_eq!(view.kinds.len(), 3);
        assert!(consumers(&BTreeMap::new()).is_none());
    }
}
