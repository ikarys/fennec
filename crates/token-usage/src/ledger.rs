use std::collections::HashSet;
use std::fmt;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use rusqlite::{Connection, params};

use crate::call::Call;

#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    Json(serde_json::Error),
    Version { found: i64, supported: i64 },
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Sqlite(e) => write!(f, "sqlite error: {e}"),
            Error::Json(e) => write!(f, "json error: {e}"),
            Error::Version { found, supported } => {
                write!(
                    f,
                    "unsupported database version {found}; supported {supported}"
                )
            }
            Error::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Sqlite(e) => Some(e),
            Error::Json(e) => Some(e),
            Error::Version { .. } => None,
            Error::Io(e) => Some(e),
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug)]
pub struct Ledger {
    pub(crate) db: Mutex<Connection>,
}

const SUPPORTED_VERSION: i64 = 1;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    external_id TEXT UNIQUE,
    at_ms INTEGER NOT NULL,
    product TEXT NOT NULL,
    group_id TEXT NOT NULL,
    group_name TEXT NOT NULL,
    run_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_tokens INTEGER NOT NULL,
    cached_tokens INTEGER NOT NULL,
    completion_tokens INTEGER NOT NULL,
    total_secs REAL NOT NULL,
    prefill_speed REAL NOT NULL,
    generation_speed REAL NOT NULL,
    breakdown TEXT NOT NULL,
    savings TEXT NOT NULL,
    labels TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_calls_at_ms ON calls(at_ms);
CREATE INDEX IF NOT EXISTS idx_calls_group_at_ms ON calls(group_id, at_ms);
CREATE INDEX IF NOT EXISTS idx_calls_run_id ON calls(run_id);
"#;

/// `record` ignores a known `external_id`; `insert_sql(true)` fills a stored call's empty
/// labels and breakdown.
fn insert_sql(fill_empty: bool) -> String {
    if fill_empty {
        format!(
            "INSERT{INSERT_COLUMNS} ON CONFLICT(external_id) DO UPDATE SET \
             labels = CASE WHEN calls.labels = '{{}}' THEN excluded.labels \
             ELSE calls.labels END, \
             breakdown = CASE WHEN calls.breakdown = '{{}}' THEN excluded.breakdown \
             ELSE calls.breakdown END \
             WHERE (calls.labels = '{{}}' AND excluded.labels != '{{}}') \
             OR (calls.breakdown = '{{}}' AND excluded.breakdown != '{{}}')"
        )
    } else {
        format!("INSERT OR IGNORE{INSERT_COLUMNS}")
    }
}

const INSERT_COLUMNS: &str = r#" INTO calls (
    external_id,
    at_ms,
    product,
    group_id,
    group_name,
    run_id,
    kind,
    model,
    prompt_tokens,
    cached_tokens,
    completion_tokens,
    total_secs,
    prefill_speed,
    generation_speed,
    breakdown,
    savings,
    labels
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
"#;

impl Ledger {
    pub fn open(path: &Path) -> Result<Self, Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;
        Self::configure(&conn)?;

        Ok(Self {
            db: Mutex::new(conn),
        })
    }

    pub fn open_in_memory() -> Result<Self, Error> {
        let conn = Connection::open_in_memory()?;
        Self::configure(&conn)?;

        Ok(Self {
            db: Mutex::new(conn),
        })
    }

    pub fn record(&self, call: &Call) -> Result<bool, Error> {
        let conn = self.db.lock().unwrap_or_else(PoisonError::into_inner);
        let inserted = write(&conn, &insert_sql(false), call)?;
        Ok(inserted > 0)
    }

    /// Stores calls not seen yet (by `external_id`). A stored call gets the imported labels and
    /// breakdown it lacked, so an importer that learned to break down its calls can fill in its
    /// past rows; nothing else of a stored call ever changes. Returns the rows inserted or filled.
    pub fn import<I: IntoIterator<Item = Call>>(&self, calls: I) -> Result<usize, Error> {
        let mut conn = self.db.lock().unwrap_or_else(PoisonError::into_inner);
        let tx = conn.transaction()?;

        let mut inserted = 0usize;
        for call in calls {
            inserted += write(&tx, &insert_sql(true), &call)?;
        }

        tx.commit()?;
        Ok(inserted)
    }

    /// The external ids of `product`'s calls that already have a breakdown, so an
    /// importer computes breakdowns only for the calls still lacking one.
    pub fn external_ids_with_breakdown(&self, product: &str) -> Result<HashSet<String>, Error> {
        let conn = self.db.lock().unwrap_or_else(PoisonError::into_inner);
        let mut stmt = conn.prepare(
            "SELECT external_id FROM calls WHERE product = ?1 \
             AND external_id IS NOT NULL AND breakdown != '{}'",
        )?;
        let rows = stmt.query_map(params![product], |row| row.get(0))?;
        let ids = rows.collect::<Result<HashSet<String>, _>>()?;
        Ok(ids)
    }

    fn configure(conn: &Connection) -> Result<(), Error> {
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        conn.execute_batch("PRAGMA busy_timeout = 5000;")?;

        let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

        if version > SUPPORTED_VERSION {
            return Err(Error::Version {
                found: version,
                supported: SUPPORTED_VERSION,
            });
        }

        conn.execute_batch(SCHEMA)?;

        if version == 0 {
            conn.execute_batch("PRAGMA user_version = 1;")?;
        }

        Ok(())
    }
}

fn write(conn: &Connection, sql: &str, call: &Call) -> Result<usize, Error> {
    let breakdown = serde_json::to_string(&call.breakdown)?;
    let savings = serde_json::to_string(&call.savings)?;
    let labels = serde_json::to_string(&call.labels)?;

    let inserted = conn.execute(
        sql,
        params![
            call.external_id.as_deref(),
            call.at_ms,
            call.product.as_str(),
            call.group_id.as_str(),
            call.group_name.as_str(),
            call.run_id.as_str(),
            call.kind.as_str(),
            call.model.as_str(),
            call.usage.prompt_tokens as i64,
            call.usage.cached_tokens as i64,
            call.usage.completion_tokens as i64,
            call.total_secs,
            call.prefill_speed,
            call.generation_speed,
            breakdown,
            savings,
            labels
        ],
    )?;

    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call::Usage;
    use crate::query::{Filter, GroupBy};
    use std::collections::BTreeMap;

    fn call_with_id(id: &str) -> Call {
        Call {
            external_id: Some(id.to_string()),
            at_ms: 1,
            product: "test".into(),
            group_id: "g".into(),
            group_name: "G".into(),
            run_id: "r".into(),
            kind: "k".into(),
            model: "m".into(),
            usage: Usage {
                prompt_tokens: 1,
                cached_tokens: 0,
                completion_tokens: 1,
            },
            total_secs: 1.0,
            prefill_speed: 1.0,
            generation_speed: 1.0,
            breakdown: BTreeMap::new(),
            savings: BTreeMap::new(),
            labels: BTreeMap::new(),
        }
    }

    fn call_with_product(product: &str) -> Call {
        let mut call = call_with_id("x");
        call.external_id = None;
        call.product = product.into();
        call
    }

    #[test]
    fn record_deduplicates_external_id() {
        let ledger = Ledger::open_in_memory().unwrap();
        let call = call_with_id("id1");

        assert!(ledger.record(&call).unwrap());
        assert!(!ledger.record(&call).unwrap());
    }

    #[test]
    fn import_counts_only_inserted_rows() {
        let ledger = Ledger::open_in_memory().unwrap();

        let a = call_with_id("a");
        let b = call_with_id("b");
        let a_again = call_with_id("a");

        let inserted = ledger.import([a, b, a_again]).unwrap();

        assert_eq!(inserted, 2);
        assert!(!ledger.record(&call_with_id("a")).unwrap());
    }

    #[test]
    fn import_labels_a_call_stored_without_labels_and_nothing_else() {
        let ledger = Ledger::open_in_memory().unwrap();
        ledger.record(&call_with_id("a")).unwrap();

        let mut labelled = call_with_id("a");
        labelled.labels.insert("messageId".into(), "m".into());
        labelled.usage.prompt_tokens = 999;
        assert_eq!(ledger.import([labelled.clone()]).unwrap(), 1);
        // Labels already there are kept.
        labelled.labels.insert("messageId".into(), "other".into());
        assert_eq!(ledger.import([labelled]).unwrap(), 0);

        let calls = ledger.calls(&Filter::default()).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].labels["messageId"], "m");
        assert_eq!(calls[0].usage.prompt_tokens, 1, "only labels are filled in");
    }

    #[test]
    fn import_breakdown_fills_empty_but_never_replaces_non_empty() {
        let ledger = Ledger::open_in_memory().unwrap();
        ledger.record(&call_with_id("a")).unwrap();

        let mut filled = call_with_id("a");
        filled.breakdown.insert("prompt/base".into(), 10);
        assert_eq!(ledger.import([filled.clone()]).unwrap(), 1);

        let mut other = call_with_id("a");
        other.breakdown.insert("prompt/other".into(), 20);
        assert_eq!(ledger.import([other]).unwrap(), 0);

        let summary = ledger.summary(&Filter::default()).unwrap();
        assert_eq!(summary.calls, 1);
        assert_eq!(summary.breakdown["prompt/base"], 10);
        assert!(!summary.breakdown.contains_key("prompt/other"));
    }

    #[test]
    fn external_ids_with_breakdown_returns_only_filled_product_ids() {
        let ledger = Ledger::open_in_memory().unwrap();

        let mut with = call_with_id("with");
        with.breakdown.insert("prompt/base".into(), 1);

        let without = call_with_id("without");

        let mut other = call_with_id("other");
        other.product = "other".into();
        other.breakdown.insert("prompt/base".into(), 1);

        ledger.record(&with).unwrap();
        ledger.record(&without).unwrap();
        ledger.record(&other).unwrap();

        assert_eq!(
            ledger.external_ids_with_breakdown("test").unwrap(),
            HashSet::from(["with".to_string()])
        );
        assert_eq!(
            ledger.external_ids_with_breakdown("other").unwrap(),
            HashSet::from(["other".to_string()])
        );
    }

    #[test]
    fn import_rolls_back_as_one_transaction() {
        let ledger = Ledger::open_in_memory().unwrap();

        {
            let conn = ledger.db.lock().unwrap();
            conn.execute(
                "CREATE TEMP TRIGGER fail_bad BEFORE INSERT ON main.calls WHEN NEW.product = 'bad' BEGIN SELECT RAISE(ABORT, 'bad product'); END",
                [],
            )
            .unwrap();
        }

        let good = call_with_product("good");
        let bad = call_with_product("bad");

        assert!(ledger.import([good, bad]).is_err());

        let summary = ledger.summary(&Filter::default()).unwrap();
        assert_eq!(summary.calls, 0);
    }

    #[test]
    fn open_creates_parent_directory() {
        let path = std::env::temp_dir().join(format!(
            "usage-ledger-parent-{}/nested/ledger.db",
            std::process::id()
        ));

        let _ = std::fs::remove_dir_all(path.parent().unwrap());

        let ledger = Ledger::open(&path).unwrap();
        drop(ledger);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn open_file_twice_keeps_data() {
        let path =
            std::env::temp_dir().join(format!("usage-ledger-file-{}.db", std::process::id()));

        let _ = std::fs::remove_file(&path);

        let ledger = Ledger::open(&path).unwrap();
        ledger.record(&call_with_id("x")).unwrap();
        drop(ledger);

        let ledger = Ledger::open(&path).unwrap();
        let summary = ledger.summary(&Filter::default()).unwrap();
        assert_eq!(summary.calls, 1);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn open_refuses_user_version_2() {
        let path =
            std::env::temp_dir().join(format!("usage-ledger-version-{}.db", std::process::id()));

        let _ = std::fs::remove_file(&path);

        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA user_version = 2;").unwrap();
        }

        let err = Ledger::open(&path).unwrap_err();

        match err {
            Error::Version { found, supported } => assert_eq!((found, supported), (2, 1)),
            other => panic!("expected version error, got {other:?}"),
        }

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn groups_are_independent() {
        let ledger = Ledger::open_in_memory().unwrap();

        let mut a = call_with_id("a");
        a.group_id = "a".into();

        let mut b = call_with_id("b");
        b.group_id = "b".into();

        ledger.record(&a).unwrap();
        ledger.record(&b).unwrap();

        let filter_a = Filter {
            group_id: Some("a".into()),
            ..Default::default()
        };
        let filter_b = Filter {
            group_id: Some("b".into()),
            ..Default::default()
        };

        assert_eq!(ledger.summary(&filter_a).unwrap().calls, 1);
        assert_eq!(ledger.summary(&filter_b).unwrap().calls, 1);

        let rows = ledger.series(&Filter::default(), GroupBy::Group).unwrap();
        assert_eq!(rows.len(), 2);
    }
}
