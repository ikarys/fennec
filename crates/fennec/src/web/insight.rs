//! The Insight page, computed here so the UI only draws it (`GET /api/insight`):
//! the usage ledger over a period, for every project or one. The ledger
//! outlives the sessions, so a deleted session still counts.

use serde::Serialize;
use serde_json::json;
use std::net::TcpStream;
use token_usage::display::{duration, percent, tokens, usd};
use token_usage::{CallRow, Filter, GroupBy, Ledger, Row, Summary, group_rows, summary_from_rows};

use super::App;
use super::context_view::{CompositionView, composition_view};
use super::http::{Request, error, send_json};
use super::pricing::{Pricing, priced, prices};
use crate::domain::clock::now_ms;
use crate::domain::context_parts::PartKind;
use crate::domain::lang::Lang;
use llm_providers::Price;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CardKind {
    Calls,
    Input,
    Cache,
    LostCache,
    Output,
    Cost,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardView {
    /// The stable id the UI keys its icon on; `label` is display text.
    pub kind: CardKind,
    pub label: String,
    pub value: String,
    pub detail: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayView {
    pub day: String,
    /// The axis label, empty on bars between labelled ones.
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
    pub project_models: BTreeMap<String, Vec<Row>>,
}

impl InsightData {
    pub fn view(
        self,
        project: Option<&str>,
        prices: &BTreeMap<String, Price>,
        lang: Lang,
    ) -> InsightView {
        let InsightData {
            total,
            days,
            projects,
            models,
            groups,
            project_models,
        } = self;

        let pricing = priced(&models, prices);

        let cards = cards(&total, &pricing, lang);
        let consumers = consumers(&total.breakdown, lang);

        let max = days
            .iter()
            .map(|row| row.summary.prompt_tokens)
            .max()
            .unwrap_or(0)
            .max(1);
        let step = days.len().div_ceil(10).max(1);
        let days = days
            .iter()
            .enumerate()
            .map(|(index, row)| day_view(row, max, index % step == 0, lang))
            .collect();

        let projects = if project.is_none() {
            let mut rows = projects;
            busiest_first(&mut rows);
            rows.into_iter()
                .map(|row| {
                    let cost = project_models
                        .get(&row.key)
                        .map(|rows| priced(rows, prices).value(lang.locale()))
                        .unwrap_or_else(|| "—".to_string());
                    row_view(&row, false, &cost, lang)
                })
                .collect()
        } else {
            Vec::new()
        };

        let mut models = models;
        busiest_first(&mut models);
        let models = models
            .into_iter()
            .map(|row| {
                let cost = prices
                    .get(&row.key)
                    .map(|price| {
                        usd(
                            price.cost(
                                row.summary.prompt_tokens,
                                row.summary.cached_tokens,
                                row.summary.completion_tokens,
                            ),
                            lang.locale(),
                        )
                    })
                    .unwrap_or_else(|| "—".to_string());
                row_view(&row, true, &cost, lang)
            })
            .collect();

        let project_options = project_options(&groups);

        InsightView {
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

    // Two scans: the period's calls, and (for the picker) every project. Every
    // figure below is grouped in memory from the period's rows, so one project
    // never triggers an extra query.
    let period_rows = ledger.rows(&base).map_err(|e| e.to_string())?;
    let picker_rows = ledger
        .rows(&Filter {
            product: Some("fennec".to_string()),
            ..Default::default()
        })
        .map_err(|e| e.to_string())?;

    let total = summary_from_rows(&period_rows, &base);
    let days = group_rows(&period_rows, &base, GroupBy::Day);
    let projects = if project.is_none() {
        group_rows(&period_rows, &base, GroupBy::Group)
    } else {
        Vec::new()
    };
    let models = group_rows(&period_rows, &base, GroupBy::Model);
    let groups = group_rows(&picker_rows, &Filter::default(), GroupBy::Group);

    let mut project_models: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for row in &projects {
        let series = project_model_series(&period_rows, &row.key);
        project_models.insert(row.key.clone(), series);
    }

    let data = InsightData {
        total,
        days,
        projects,
        models,
        groups,
        project_models,
    };

    let preferences = app.store()?.preferences()?;
    let prices = prices(&preferences);
    // The human's display language drives the server-built texts.
    let lang = Lang::from_code(&preferences.display.language);

    send_json(
        stream,
        200,
        &json!(data.view(project.as_deref(), &prices, lang)),
    )
}

/// One project's model breakdown, from the period's rows. `group_rows` only
/// applies the label filter, so the `group_id` scoping a DB query would have
/// done is applied to the rows here.
fn project_model_series(rows: &[CallRow], group_id: &str) -> Vec<Row> {
    let scoped: Vec<CallRow> = rows
        .iter()
        .filter(|r| r.group_id == group_id)
        .cloned()
        .collect();
    group_rows(&scoped, &Filter::default(), GroupBy::Model)
}

fn busiest_first(rows: &mut [Row]) {
    rows.sort_by(|a, b| {
        b.summary
            .prompt_tokens
            .cmp(&a.summary.prompt_tokens)
            .then_with(|| a.key.cmp(&b.key))
    });
}

fn cards(total: &Summary, pricing: &Pricing, lang: Lang) -> Vec<CardView> {
    let fresh = total.prompt_tokens.saturating_sub(total.cached_tokens);
    let cache = percent(total.cached_tokens, total.prompt_tokens);
    let lost_detail = match (lang, total.lost_cache_calls) {
        (Lang::En, 0) => "no call re-read its prefix".to_string(),
        (Lang::Fr, 0) => "aucun appel n'a relu son préfixe".to_string(),
        (Lang::En, n) => format!("{n} calls re-read what the server had already seen"),
        (Lang::Fr, n) => format!("{n} appels ont relu ce que le serveur avait déjà vu"),
    };

    vec![
        CardView {
            kind: CardKind::Calls,
            label: lang.pick("Calls", "Appels").into(),
            value: total.calls.to_string(),
            detail: match lang {
                Lang::En => format!("model time {}", duration(total.total_secs, lang.locale())),
                Lang::Fr => format!("temps modèle {}", duration(total.total_secs, lang.locale())),
            },
        },
        CardView {
            kind: CardKind::Input,
            label: lang.pick("Input", "Entrée").into(),
            value: tokens(total.prompt_tokens, lang.locale()),
            detail: match lang {
                Lang::En => format!("{} uncached", tokens(fresh, lang.locale())),
                Lang::Fr => format!("{} hors cache", tokens(fresh, lang.locale())),
            },
        },
        CardView {
            kind: CardKind::Cache,
            label: lang.pick("Cache", "Cache").into(),
            value: format!("{cache} %"),
            detail: match lang {
                Lang::En => format!(
                    "{} read from cache",
                    tokens(total.cached_tokens, lang.locale())
                ),
                Lang::Fr => format!(
                    "{} lus du cache",
                    tokens(total.cached_tokens, lang.locale())
                ),
            },
        },
        CardView {
            kind: CardKind::LostCache,
            label: lang.pick("Lost cache", "Cache perdu").into(),
            value: tokens(total.lost_cache_tokens, lang.locale()),
            detail: lost_detail,
        },
        CardView {
            kind: CardKind::Output,
            label: lang.pick("Output", "Sortie").into(),
            value: tokens(total.completion_tokens, lang.locale()),
            detail: lang.pick("generated tokens", "tokens générés").to_string(),
        },
        CardView {
            kind: CardKind::Cost,
            label: lang.pick("Estimated cost", "Coût estimé").into(),
            value: pricing.value(lang.locale()),
            detail: pricing.detail(lang),
        },
    ]
}

/// The ledger's `<kind>/<label>` keys, largest first; keys of another
/// shape (an older or foreign writer) are left out.
fn consumers(breakdown: &BTreeMap<String, u64>, lang: Lang) -> Option<CompositionView> {
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
    Some(composition_view(&shares, total, 0, lang))
}

fn day_view(row: &Row, max: u64, show_label: bool, lang: Lang) -> DayView {
    let prompt = row.summary.prompt_tokens;
    let cached = row.summary.cached_tokens;
    let lost = row
        .summary
        .lost_cache_tokens
        .min(prompt.saturating_sub(cached));
    let fresh = prompt.saturating_sub(cached).saturating_sub(lost);

    let label = day_label(&row.key);
    let axis_label = if show_label {
        label.clone()
    } else {
        String::new()
    };
    let calls = row.summary.calls;
    let input = tokens(prompt, lang.locale());
    let cache = percent(cached, prompt);
    let lost_text = tokens(lost, lang.locale());
    let output = tokens(row.summary.completion_tokens, lang.locale());

    let title = match lang {
        Lang::En => format!(
            "{label} · {calls} calls · input {input} (cache {cache} %) · lost cache {lost_text} · output {output}"
        ),
        Lang::Fr => format!(
            "{label} · {calls} appels · entrée {input} (cache {cache} %) · cache perdu {lost_text} · sortie {output}"
        ),
    };

    DayView {
        day: row.key.clone(),
        label: axis_label,
        cached_percent: percent(cached, max),
        fresh_percent: percent(fresh, max),
        lost_percent: percent(lost, max),
        title,
    }
}

fn row_view(row: &Row, model: bool, cost: &str, lang: Lang) -> RowView {
    let name = if model {
        row.key.clone()
    } else {
        row.name.clone()
    };
    let cache = percent(row.summary.cached_tokens, row.summary.prompt_tokens);
    let lost = if row.summary.lost_cache_tokens == 0 {
        "—".to_string()
    } else {
        tokens(row.summary.lost_cache_tokens, lang.locale())
    };

    RowView {
        key: row.key.clone(),
        name,
        calls: row.summary.calls.to_string(),
        input: tokens(row.summary.prompt_tokens, lang.locale()),
        cache: format!("{cache} %"),
        output: tokens(row.summary.completion_tokens, lang.locale()),
        lost,
        cost: cost.to_string(),
        time: duration(row.summary.total_secs, lang.locale()),
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

    fn call_row(id: i64, group_id: &str, model: &str, prompt: u64) -> CallRow {
        CallRow {
            id,
            at_ms: 0,
            group_id: group_id.to_string(),
            group_name: group_id.to_string(),
            model: model.to_string(),
            kind: String::new(),
            run_id: String::new(),
            prompt_tokens: prompt,
            cached_tokens: 0,
            completion_tokens: 0,
            total_secs: 0.0,
            breakdown: Default::default(),
            savings: Default::default(),
            labels: Default::default(),
            prefill_speed: 0.0,
            generation_speed: 0.0,
            lost_cache_tokens: 0,
        }
    }

    #[test]
    fn project_model_series_scopes_to_its_project() {
        // Two projects, one shared model: the overview must not show each
        // project the combined model total.
        let rows = vec![
            call_row(1, "p1", "m", 100),
            call_row(2, "p1", "m", 50),
            call_row(3, "p2", "m", 1_000),
        ];

        let p1 = project_model_series(&rows, "p1");
        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].key, "m");
        assert_eq!(p1[0].summary.prompt_tokens, 150, "only p1's calls");

        let p2 = project_model_series(&rows, "p2");
        assert_eq!(p2[0].summary.prompt_tokens, 1_000, "only p2's calls");
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

        let prices = BTreeMap::from([("m1".to_string(), Price::default())]);
        let view = InsightData {
            total,
            days,
            projects,
            models,
            groups,
            project_models: BTreeMap::new(),
        }
        .view(None, &prices, Lang::Fr);

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
            project_models: BTreeMap::new(),
        }
        .view(Some("p1"), &BTreeMap::new(), Lang::Fr);

        assert!(view.projects.is_empty());
    }

    #[test]
    fn cost_card_without_any_price_is_a_dash() {
        let total = summary(1, 1000, 0, 10, 0, 0, 1.0);
        let models = vec![row("m1", "m1", summary(1, 1000, 0, 10, 0, 0, 1.0))];
        let pricing = priced(&models, &BTreeMap::new());
        let cards = cards(&total, &pricing, Lang::Fr);

        let cost = cards
            .iter()
            .find(|card| card.kind == CardKind::Cost)
            .expect("cost card");
        assert_eq!(cost.value, "—");
        assert_eq!(
            cost.detail,
            "aucun modèle n'a de prix : Paramètres, Modèles"
        );
    }

    #[test]
    fn view_in_english() {
        let total = summary(10, 1000, 970, 100, 0, 0, 1.0);
        let view = InsightData {
            total,
            days: vec![],
            projects: vec![],
            models: vec![],
            groups: vec![],
            project_models: BTreeMap::new(),
        }
        .view(None, &BTreeMap::new(), Lang::En);

        let kinds: Vec<CardKind> = view.cards.iter().map(|card| card.kind).collect();
        assert_eq!(
            kinds,
            vec![
                CardKind::Calls,
                CardKind::Input,
                CardKind::Cache,
                CardKind::LostCache,
                CardKind::Output,
                CardKind::Cost
            ]
        );
        assert_eq!(json!(view.cards[3])["kind"], "lostCache");
        let labels: Vec<&str> = view.cards.iter().map(|card| card.label.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                "Calls",
                "Input",
                "Cache",
                "Lost cache",
                "Output",
                "Estimated cost"
            ]
        );
        assert_eq!(
            view.cards[5].detail,
            "no model has a price: Settings, Models"
        );
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
        let view = consumers(&breakdown, Lang::Fr).expect("consumers");

        let labels: Vec<&str> = view.parts.iter().map(|p| p.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Résultat : read_file", "AGENTS.md", "Messages humains"]
        );
        assert_eq!(view.parts[0].percent, 60);
        assert_eq!(view.kinds.len(), 3);
        assert!(consumers(&BTreeMap::new(), Lang::Fr).is_none());
    }

    #[test]
    fn day_labels_are_sparse_for_many_days() {
        let days: Vec<Row> = (0..25)
            .map(|i| {
                let key = format!("2026-10-{:02}", i + 1);
                row(&key, &key, summary(1, 100, 0, 10, 0, 0, 1.0))
            })
            .collect();
        let total = summary(25, 2500, 0, 250, 0, 0, 25.0);

        let view = InsightData {
            total,
            days,
            projects: vec![],
            models: vec![],
            groups: vec![],
            project_models: BTreeMap::new(),
        }
        .view(None, &BTreeMap::new(), Lang::Fr);

        for (index, day) in view.days.iter().enumerate() {
            assert_eq!(day.label.is_empty(), index % 3 != 0);
            assert!(day.title.starts_with(&day_label(&day.day)));
        }

        assert_eq!(view.days[0].label, "01/10");
        assert_eq!(view.days[3].label, "04/10");
        assert_eq!(view.days[24].label, "25/10");
    }
}
