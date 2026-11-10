use std::collections::{BTreeMap, HashMap};
use std::sync::PoisonError;

use rusqlite::{ToSql, params_from_iter};
use serde::Serialize;

use crate::call::Usage;
use crate::ledger::{Error, Ledger};

/// Chat templates re-render the last turn's edges, so a small gap is not a loss.
pub const LOST_CACHE_SLACK: u64 = 1_000;

#[derive(Default, Clone, Debug)]
pub struct Filter {
    pub product: Option<String>,
    pub group_id: Option<String>,
    pub run_id: Option<String>,
    pub kind: Option<String>,
    pub model: Option<String>,
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
    pub label: Option<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub calls: u64,
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub completion_tokens: u64,
    pub cache_hit_rate: f64,
    /// Calls that re-read more than `LOST_CACHE_SLACK` tokens their run had already sent.
    pub lost_cache_calls: u64,
    /// The tokens those calls re-read.
    pub lost_cache_tokens: u64,
    pub total_secs: f64,
    pub breakdown: BTreeMap<String, u64>,
    pub savings: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GroupBy {
    Day,
    Group,
    Model,
    Kind,
    Run,
}

/// The speed figures of one call.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestCall {
    pub at_ms: i64,
    pub model: String,
    pub prefill_speed: f64,
    pub generation_speed: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub key: String,
    pub name: String,
    pub summary: Summary,
}

/// One stored call, as read back.
#[derive(Debug, Clone, PartialEq)]
pub struct CallRecord {
    pub at_ms: i64,
    pub model: String,
    pub kind: String,
    pub run_id: String,
    pub usage: Usage,
    pub total_secs: f64,
    pub prefill_speed: f64,
    pub generation_speed: f64,
    pub labels: BTreeMap<String, String>,
}

struct RawRow {
    id: i64,
    at_ms: i64,
    group_id: String,
    group_name: String,
    model: String,
    kind: String,
    run_id: String,
    prompt_tokens: i64,
    cached_tokens: i64,
    completion_tokens: i64,
    total_secs: f64,
    breakdown: String,
    savings: String,
    labels: String,
    prefill_speed: f64,
    generation_speed: f64,
}

/// One stored call, read back for in-memory grouping: usage, the per-part
/// `breakdown`/`savings`, labels, and the `lost_cache_tokens` computed in time
/// order. Oldest first; `id` is the ordering tie-break.
#[derive(Clone)]
pub struct CallRow {
    pub id: i64,
    pub at_ms: i64,
    pub group_id: String,
    pub group_name: String,
    pub model: String,
    pub kind: String,
    pub run_id: String,
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub completion_tokens: u64,
    pub total_secs: f64,
    pub breakdown: BTreeMap<String, u64>,
    pub savings: BTreeMap<String, u64>,
    pub labels: BTreeMap<String, String>,
    pub prefill_speed: f64,
    pub generation_speed: f64,
    pub lost_cache_tokens: u64,
}

/// One group (a project) and its most recent name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: String,
    pub name: String,
}

impl Ledger {
    pub fn summary(&self, f: &Filter) -> Result<Summary, Error> {
        Ok(summary_from_rows(&self.rows(f)?, f))
    }

    /// Every call matching `f`, oldest first, with its labels.
    pub fn calls(&self, f: &Filter) -> Result<Vec<CallRecord>, Error> {
        Ok(self
            .rows(f)?
            .into_iter()
            .filter(|r| matches_label(&r.labels, f))
            .map(|r| CallRecord {
                at_ms: r.at_ms,
                model: r.model,
                kind: r.kind,
                run_id: r.run_id,
                usage: Usage {
                    prompt_tokens: r.prompt_tokens,
                    cached_tokens: r.cached_tokens,
                    completion_tokens: r.completion_tokens,
                },
                total_secs: r.total_secs,
                prefill_speed: r.prefill_speed,
                generation_speed: r.generation_speed,
                labels: r.labels,
            })
            .collect())
    }

    /// The most recent call matching `f`, for "how fast was the last one".
    /// ponytail: loads the matching rows, fine for one run; a LIMIT 1 query
    /// when a filter can match a whole product.
    pub fn latest(&self, f: &Filter) -> Result<Option<LatestCall>, Error> {
        Ok(self
            .rows(f)?
            .into_iter()
            .rev()
            .find(|r| matches_label(&r.labels, f))
            .map(|r| LatestCall {
                at_ms: r.at_ms,
                model: r.model,
                prefill_speed: r.prefill_speed,
                generation_speed: r.generation_speed,
            }))
    }

    pub fn series(&self, f: &Filter, by: GroupBy) -> Result<Vec<Row>, Error> {
        Ok(group_rows(&self.rows(f)?, f, by))
    }

    /// Every group a product has calls for, with its most recent name, by id.
    /// One SQL scan: a project picker must not load every call ever recorded.
    pub fn groups(&self, product: &str) -> Result<Vec<Group>, Error> {
        let conn = self.db.lock().unwrap_or_else(PoisonError::into_inner);
        // SQLite fills the bare column `group_name` from the row holding MAX(at_ms).
        let mut stmt = conn.prepare(
            "SELECT group_id, group_name, MAX(at_ms) FROM calls WHERE product = ?1 GROUP BY group_id ORDER BY group_id",
        )?;
        let groups = stmt
            .query_map([product], |row| {
                Ok(Group {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(groups)
    }

    /// Every call matching `f`, oldest first, with its `breakdown`, `savings`,
    /// labels and computed `lost_cache_tokens`, for in-memory grouping.
    pub fn rows(&self, f: &Filter) -> Result<Vec<CallRow>, Error> {
        let conn = self.db.lock().unwrap_or_else(PoisonError::into_inner);
        let (sql, params) = build_query(f);

        let mut stmt = conn.prepare(&sql)?;
        let raw_rows = stmt.query_map(params_from_iter(params.iter()), |row| {
            Ok(RawRow {
                id: row.get(0)?,
                at_ms: row.get(1)?,
                group_id: row.get(2)?,
                group_name: row.get(3)?,
                model: row.get(4)?,
                kind: row.get(5)?,
                run_id: row.get(6)?,
                prompt_tokens: row.get(7)?,
                cached_tokens: row.get(8)?,
                completion_tokens: row.get(9)?,
                total_secs: row.get(10)?,
                breakdown: row.get(11)?,
                savings: row.get(12)?,
                labels: row.get(13)?,
                prefill_speed: row.get(14)?,
                generation_speed: row.get(15)?,
            })
        })?;

        let mut out = Vec::new();
        for raw in raw_rows {
            let raw = raw?;
            out.push(CallRow {
                id: raw.id,
                at_ms: raw.at_ms,
                group_id: raw.group_id,
                group_name: raw.group_name,
                model: raw.model,
                kind: raw.kind,
                run_id: raw.run_id,
                prompt_tokens: raw.prompt_tokens as u64,
                cached_tokens: raw.cached_tokens as u64,
                completion_tokens: raw.completion_tokens as u64,
                total_secs: raw.total_secs,
                breakdown: serde_json::from_str(&raw.breakdown)?,
                savings: serde_json::from_str(&raw.savings)?,
                labels: serde_json::from_str(&raw.labels)?,
                prefill_speed: raw.prefill_speed,
                generation_speed: raw.generation_speed,
                lost_cache_tokens: 0,
            });
        }

        mark_lost_cache(&mut out);

        Ok(out)
    }
}

/// The summary of the matching rows, in memory.
pub fn summary_from_rows(rows: &[CallRow], f: &Filter) -> Summary {
    let mut summary = new_summary();
    for row in rows {
        if matches_label(&row.labels, f) {
            add_to_summary(&mut summary, row);
        }
    }

    finish_summary(&mut summary);
    summary
}

/// The matching rows grouped by `by`, in memory.
pub fn group_rows(rows: &[CallRow], f: &Filter, by: GroupBy) -> Vec<Row> {
    let mut groups: BTreeMap<String, Summary> = BTreeMap::new();
    let mut latest_group: BTreeMap<String, (i64, i64, String)> = BTreeMap::new();
    let mut day_span: Option<(i64, i64)> = None;

    for row in rows {
        if !matches_label(&row.labels, f) {
            continue;
        }

        let key = match by {
            GroupBy::Day => {
                let day = row.at_ms.div_euclid(86_400_000);
                day_span = Some(day_span.map_or((day, day), |(lo, hi)| (lo.min(day), hi.max(day))));
                day_key(day)
            }
            GroupBy::Group => row.group_id.clone(),
            GroupBy::Model => row.model.clone(),
            GroupBy::Kind => row.kind.clone(),
            GroupBy::Run => row.run_id.clone(),
        };

        let entry = groups.entry(key.clone()).or_insert_with(new_summary);
        add_to_summary(entry, row);

        if by == GroupBy::Group {
            let current = latest_group.entry(row.group_id.clone()).or_insert((
                i64::MIN,
                i64::MIN,
                String::new(),
            ));

            if (row.at_ms, row.id) > (current.0, current.1) {
                current.0 = row.at_ms;
                current.1 = row.id;
                current.2 = row.group_name.clone();
            }
        }
    }

    // A chart reads a day series as a continuous axis: the days without a
    // call between the first and the last one are empty rows, not holes.
    if let Some((first, last)) = day_span {
        for day in first..=last {
            groups.entry(day_key(day)).or_insert_with(new_summary);
        }
    }

    let mut out = Vec::with_capacity(groups.len());
    for (key, mut summary) in groups {
        finish_summary(&mut summary);

        let name = if by == GroupBy::Group {
            latest_group
                .get(&key)
                .map(|(_, _, name)| name.clone())
                .unwrap_or_else(|| key.clone())
        } else {
            key.clone()
        };

        out.push(Row { key, name, summary });
    }

    out
}

fn build_query(f: &Filter) -> (String, Vec<Box<dyn ToSql>>) {
    let mut clauses = Vec::new();
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(product) = &f.product {
        clauses.push("product = ?".to_string());
        params.push(Box::new(product.clone()));
    }

    if let Some(group_id) = &f.group_id {
        clauses.push("group_id = ?".to_string());
        params.push(Box::new(group_id.clone()));
    }

    if let Some(run_id) = &f.run_id {
        clauses.push("run_id = ?".to_string());
        params.push(Box::new(run_id.clone()));
    }

    if let Some(kind) = &f.kind {
        clauses.push("kind = ?".to_string());
        params.push(Box::new(kind.clone()));
    }

    if let Some(model) = &f.model {
        clauses.push("model = ?".to_string());
        params.push(Box::new(model.clone()));
    }

    if let Some(since_ms) = f.since_ms {
        clauses.push("at_ms >= ?".to_string());
        params.push(Box::new(since_ms));
    }

    if let Some(until_ms) = f.until_ms {
        clauses.push("at_ms < ?".to_string());
        params.push(Box::new(until_ms));
    }

    let where_clause = if clauses.is_empty() {
        "1 = 1".to_string()
    } else {
        clauses.join(" AND ")
    };

    let sql = format!(
        "SELECT id, at_ms, group_id, group_name, model, kind, run_id, prompt_tokens, cached_tokens, completion_tokens, total_secs, breakdown, savings, labels, prefill_speed, generation_speed FROM calls WHERE {where_clause} ORDER BY at_ms ASC, id ASC"
    );

    (sql, params)
}

fn matches_label(labels: &BTreeMap<String, String>, f: &Filter) -> bool {
    match &f.label {
        Some((key, value)) => labels.get(key).is_some_and(|v| v == value),
        None => true,
    }
}

/// Mark calls that lost a prefix cache.
///
/// Rows are already in time order. A call counts only when its run has a
/// previous prompt, the prompt did not shrink, and the cache gap is above
/// `LOST_CACHE_SLACK`. A shrunken prompt is a rewrite, not a loss.
fn mark_lost_cache(rows: &mut [CallRow]) {
    let mut previous_prompt: HashMap<String, u64> = HashMap::new();

    for row in rows.iter_mut() {
        row.lost_cache_tokens = match previous_prompt.get(&row.run_id) {
            Some(&prev)
                if row.prompt_tokens >= prev
                    && prev.saturating_sub(row.cached_tokens) > LOST_CACHE_SLACK =>
            {
                prev - row.cached_tokens
            }
            _ => 0,
        };
        previous_prompt.insert(row.run_id.clone(), row.prompt_tokens);
    }
}

fn new_summary() -> Summary {
    Summary {
        calls: 0,
        prompt_tokens: 0,
        cached_tokens: 0,
        completion_tokens: 0,
        cache_hit_rate: 0.0,
        lost_cache_calls: 0,
        lost_cache_tokens: 0,
        total_secs: 0.0,
        breakdown: BTreeMap::new(),
        savings: BTreeMap::new(),
    }
}

fn add_to_summary(summary: &mut Summary, row: &CallRow) {
    summary.calls += 1;
    summary.prompt_tokens = summary.prompt_tokens.saturating_add(row.prompt_tokens);
    summary.cached_tokens = summary.cached_tokens.saturating_add(row.cached_tokens);
    summary.completion_tokens = summary
        .completion_tokens
        .saturating_add(row.completion_tokens);
    summary.total_secs += row.total_secs;

    if row.lost_cache_tokens > 0 {
        summary.lost_cache_calls += 1;
        summary.lost_cache_tokens = summary
            .lost_cache_tokens
            .saturating_add(row.lost_cache_tokens);
    }

    add_map(&mut summary.breakdown, &row.breakdown);
    add_map(&mut summary.savings, &row.savings);
}

fn add_map(target: &mut BTreeMap<String, u64>, source: &BTreeMap<String, u64>) {
    for (key, value) in source {
        target
            .entry(key.clone())
            .and_modify(|existing| *existing = existing.saturating_add(*value))
            .or_insert(*value);
    }
}

fn finish_summary(summary: &mut Summary) {
    summary.cache_hit_rate = if summary.prompt_tokens == 0 {
        0.0
    } else {
        summary.cached_tokens as f64 / summary.prompt_tokens as f64
    };
}

fn day_key(days: i64) -> String {
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    (y, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call::Call;

    fn base_call() -> Call {
        Call {
            external_id: None,
            at_ms: 1,
            product: "p".into(),
            group_id: "g".into(),
            group_name: "G".into(),
            run_id: "r".into(),
            kind: "k".into(),
            model: "m".into(),
            usage: Usage {
                prompt_tokens: 10,
                cached_tokens: 2,
                completion_tokens: 3,
            },
            total_secs: 1.0,
            prefill_speed: 1.0,
            generation_speed: 1.0,
            breakdown: BTreeMap::new(),
            savings: BTreeMap::new(),
            labels: BTreeMap::new(),
        }
    }

    #[test]
    fn groups_list_each_group_once_with_its_latest_name() {
        let ledger = Ledger::open_in_memory().unwrap();
        let mut old = base_call();
        old.group_name = "Old".into();
        ledger.record(&old).unwrap();
        let mut renamed = base_call();
        renamed.at_ms = 5;
        renamed.group_name = "New".into();
        ledger.record(&renamed).unwrap();
        let mut other = base_call();
        other.group_id = "h".into();
        ledger.record(&other).unwrap();
        let mut foreign = base_call();
        foreign.product = "x".into();
        foreign.group_id = "z".into();
        ledger.record(&foreign).unwrap();

        let groups = ledger.groups("p").unwrap();
        assert_eq!(
            groups,
            vec![
                Group {
                    id: "g".into(),
                    name: "New".into()
                },
                Group {
                    id: "h".into(),
                    name: "G".into()
                },
            ]
        );
    }

    #[test]
    fn calls_come_back_oldest_first_with_labels() {
        let ledger = Ledger::open_in_memory().unwrap();
        let mut late = base_call();
        late.at_ms = 20;
        late.labels.insert("messageId".into(), "m2".into());
        let mut early = base_call();
        early.at_ms = 10;
        early.labels.insert("messageId".into(), "m1".into());
        ledger.record(&late).unwrap();
        ledger.record(&early).unwrap();

        let calls = ledger.calls(&Filter::default()).unwrap();
        let ids: Vec<_> = calls
            .iter()
            .map(|c| c.labels["messageId"].as_str())
            .collect();
        assert_eq!(ids, ["m1", "m2"]);
        assert_eq!(calls[0].usage.prompt_tokens, 10);
    }

    #[test]
    fn rows_expose_breakdown_savings_and_lost_cache() {
        let ledger = Ledger::open_in_memory().unwrap();

        let mut c1 = base_call();
        c1.at_ms = 10;
        c1.usage = Usage {
            prompt_tokens: 5000,
            cached_tokens: 0,
            completion_tokens: 0,
        };
        c1.breakdown.insert("prompt/base".into(), 42);
        c1.savings.insert("s".into(), 7);
        c1.labels.insert("messageId".into(), "m1".into());

        let mut c2 = base_call();
        c2.at_ms = 20;
        c2.usage = Usage {
            prompt_tokens: 6000,
            cached_tokens: 0,
            completion_tokens: 0,
        };
        c2.labels.insert("messageId".into(), "m2".into());

        ledger.record(&c1).unwrap();
        ledger.record(&c2).unwrap();

        let rows = ledger.rows(&Filter::default()).unwrap();
        assert_eq!(rows.len(), 2);

        assert_eq!(rows[0].breakdown["prompt/base"], 42);
        assert_eq!(rows[0].savings["s"], 7);
        assert_eq!(rows[0].labels["messageId"], "m1");
        assert_eq!(rows[0].lost_cache_tokens, 0);

        // 6000 prompt after a 5000 already sent, nothing cached: a loss.
        assert_eq!(rows[1].lost_cache_tokens, 5000);
    }

    #[test]
    fn summary_totals_and_cache_hit_rate() {
        let ledger = Ledger::open_in_memory().unwrap();

        let mut c1 = base_call();
        c1.at_ms = 1;
        c1.usage = Usage {
            prompt_tokens: 100,
            cached_tokens: 25,
            completion_tokens: 10,
        };
        c1.total_secs = 2.0;
        c1.breakdown.insert("a".into(), 1);
        c1.savings.insert("s".into(), 2);

        let mut c2 = base_call();
        c2.at_ms = 2;
        c2.usage = Usage {
            prompt_tokens: 50,
            cached_tokens: 0,
            completion_tokens: 5,
        };
        c2.total_secs = 3.0;
        c2.breakdown.insert("a".into(), 3);
        c2.breakdown.insert("b".into(), 1);
        c2.savings.insert("s".into(), 4);

        ledger.record(&c1).unwrap();
        ledger.record(&c2).unwrap();

        let summary = ledger.summary(&Filter::default()).unwrap();

        assert_eq!(summary.calls, 2);
        assert_eq!(summary.prompt_tokens, 150);
        assert_eq!(summary.cached_tokens, 25);
        assert_eq!(summary.completion_tokens, 15);
        assert!((summary.cache_hit_rate - 25.0 / 150.0).abs() < 1e-9);
        assert_eq!(summary.total_secs, 5.0);
        assert_eq!(summary.breakdown.get("a"), Some(&4));
        assert_eq!(summary.breakdown.get("b"), Some(&1));
        assert_eq!(summary.savings.get("s"), Some(&6));
    }

    #[test]
    fn cache_hit_rate_is_zero_when_prompt_is_zero() {
        let ledger = Ledger::open_in_memory().unwrap();

        let mut call = base_call();
        call.usage = Usage {
            prompt_tokens: 0,
            cached_tokens: 5,
            completion_tokens: 1,
        };

        ledger.record(&call).unwrap();

        let summary = ledger.summary(&Filter::default()).unwrap();
        assert_eq!(summary.cache_hit_rate, 0.0);
    }

    #[test]
    fn filters_by_group_since_until_and_label() {
        let ledger = Ledger::open_in_memory().unwrap();

        let mut c1 = base_call();
        c1.group_id = "a".into();
        c1.at_ms = 100;

        let mut c2 = base_call();
        c2.group_id = "b".into();
        c2.at_ms = 200;

        let mut c3 = base_call();
        c3.group_id = "a".into();
        c3.at_ms = 300;

        let mut c4 = base_call();
        c4.at_ms = 400;
        c4.labels.insert("env".into(), "prod".into());

        ledger.record(&c1).unwrap();
        ledger.record(&c2).unwrap();
        ledger.record(&c3).unwrap();
        ledger.record(&c4).unwrap();

        let group_a = Filter {
            group_id: Some("a".into()),
            ..Default::default()
        };
        assert_eq!(ledger.summary(&group_a).unwrap().calls, 2);

        let window = Filter {
            since_ms: Some(150),
            until_ms: Some(300),
            ..Default::default()
        };
        assert_eq!(ledger.summary(&window).unwrap().calls, 1);

        let labeled = Filter {
            label: Some(("env".into(), "prod".into())),
            ..Default::default()
        };
        assert_eq!(ledger.summary(&labeled).unwrap().calls, 1);
    }

    #[test]
    fn series_by_day_uses_utc_midnight() {
        let ledger = Ledger::open_in_memory().unwrap();

        let mut before = base_call();
        before.at_ms = 1_704_153_599_999;

        let mut after = base_call();
        after.at_ms = 1_704_153_600_000;

        ledger.record(&before).unwrap();
        ledger.record(&after).unwrap();

        let rows = ledger.series(&Filter::default(), GroupBy::Day).unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].key, "2024-01-01");
        assert_eq!(rows[1].key, "2024-01-02");
    }

    #[test]
    fn series_by_group_uses_latest_group_name() {
        let ledger = Ledger::open_in_memory().unwrap();

        let mut old = base_call();
        old.at_ms = 1;
        old.group_id = "g".into();
        old.group_name = "old".into();

        let mut new = base_call();
        new.at_ms = 2;
        new.group_id = "g".into();
        new.group_name = "new".into();

        ledger.record(&old).unwrap();
        ledger.record(&new).unwrap();

        let rows = ledger.series(&Filter::default(), GroupBy::Group).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, "g");
        assert_eq!(rows[0].name, "new");
    }

    #[test]
    fn series_sums_breakdown_and_savings() {
        let ledger = Ledger::open_in_memory().unwrap();

        let mut c1 = base_call();
        c1.model = "m".into();
        c1.breakdown.insert("a".into(), 1);
        c1.savings.insert("s".into(), 2);

        let mut c2 = base_call();
        c2.model = "m".into();
        c2.breakdown.insert("a".into(), 3);
        c2.savings.insert("s".into(), 4);

        ledger.record(&c1).unwrap();
        ledger.record(&c2).unwrap();

        let rows = ledger.series(&Filter::default(), GroupBy::Model).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, "m");
        assert_eq!(rows[0].summary.breakdown.get("a"), Some(&4));
        assert_eq!(rows[0].summary.savings.get("s"), Some(&6));
    }

    #[test]
    fn latest_is_the_most_recent_matching_call() {
        let ledger = Ledger::open_in_memory().unwrap();
        assert_eq!(ledger.latest(&Filter::default()).unwrap(), None);

        let mut old = base_call();
        old.at_ms = 1_000;
        old.generation_speed = 10.0;
        let mut new = base_call();
        new.at_ms = 2_000;
        new.generation_speed = 42.0;
        new.run_id = "other".into();
        ledger.record(&old).unwrap();
        ledger.record(&new).unwrap();

        let any = ledger.latest(&Filter::default()).unwrap().unwrap();
        assert_eq!(any.generation_speed, 42.0);
        let only_old = Filter {
            run_id: Some(old.run_id.clone()),
            ..Filter::default()
        };
        assert_eq!(ledger.latest(&only_old).unwrap().unwrap().at_ms, 1_000);
    }

    #[test]
    fn lost_cache_counts_gaps_in_growing_prompts() {
        let ledger = Ledger::open_in_memory().unwrap();

        fn call(run_id: &str, at_ms: i64, prompt: u64, cached: u64) -> Call {
            let mut call = base_call();
            call.run_id = run_id.into();
            call.at_ms = at_ms;
            call.usage = Usage {
                prompt_tokens: prompt,
                cached_tokens: cached,
                completion_tokens: 0,
            };
            call
        }

        ledger.record(&call("r", 10, 1000, 0)).unwrap();
        ledger.record(&call("r", 20, 3000, 900)).unwrap();
        ledger.record(&call("b", 25, 5000, 0)).unwrap();
        ledger.record(&call("r", 30, 6000, 500)).unwrap();
        ledger.record(&call("r", 40, 2000, 0)).unwrap();
        ledger.record(&call("r", 50, 2500, 1900)).unwrap();

        let summary = ledger.summary(&Filter::default()).unwrap();
        assert_eq!(summary.lost_cache_calls, 1);
        assert_eq!(summary.lost_cache_tokens, 2500);

        let rows = ledger.series(&Filter::default(), GroupBy::Run).unwrap();
        let run_a = rows.iter().find(|row| row.key == "r").unwrap();
        assert_eq!(run_a.summary.lost_cache_calls, 1);
        assert_eq!(run_a.summary.lost_cache_tokens, 2500);

        let run_b = rows.iter().find(|row| row.key == "b").unwrap();
        assert_eq!(run_b.summary.lost_cache_calls, 0);
        assert_eq!(run_b.summary.lost_cache_tokens, 0);
    }

    #[test]
    fn series_by_day_fills_missing_days() {
        let ledger = Ledger::open_in_memory().unwrap();

        let mut first = base_call();
        first.at_ms = 1_704_067_200_000;

        let mut third = base_call();
        third.at_ms = 1_704_240_000_000;

        ledger.record(&first).unwrap();
        ledger.record(&third).unwrap();

        let rows = ledger.series(&Filter::default(), GroupBy::Day).unwrap();

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].key, "2024-01-01");
        assert_eq!(rows[1].key, "2024-01-02");
        assert_eq!(rows[1].name, "2024-01-02");
        assert_eq!(rows[1].summary.calls, 0);
        assert_eq!(rows[2].key, "2024-01-03");
    }
}
