//! Ordered, numbered SQLite schema migrations recorded in `PRAGMA user_version`.

use rusqlite::Connection;

/// Append a step for every schema change; never edit or reorder a released step: stored databases depend on them.
const STEPS: &[fn(&Connection) -> rusqlite::Result<()>] = &[baseline];

fn baseline(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch(super::SCHEMA)?;
    db.execute_batch(super::ideas::SCHEMA)?;
    db.execute_batch(super::plugins::SCHEMA)?;

    if !has_column(db, "sessions", "parent_id")? {
        db.execute("ALTER TABLE sessions ADD COLUMN parent_id TEXT", [])?;
    }
    if !has_column(db, "projects", "favorite")? {
        db.execute(
            "ALTER TABLE projects ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    if !has_column(db, "sessions", "linked_project_id")? {
        db.execute("ALTER TABLE sessions ADD COLUMN linked_project_id TEXT", [])?;
    }

    Ok(())
}

pub(super) fn migrate(db: &Connection) -> rusqlite::Result<()> {
    let current: i64 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if current > STEPS.len() as i64 {
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISMATCH),
            Some(format!(
                "database user_version {current} is newer than fennec's last migration step {}",
                STEPS.len()
            )),
        ));
    }

    let current = current.max(0);

    for version in current..STEPS.len() as i64 {
        db.execute_batch("BEGIN IMMEDIATE")?;

        let result = (|| -> rusqlite::Result<()> {
            let step = STEPS[version as usize];
            step(db)?;
            db.execute_batch(&format!("PRAGMA user_version = {};", version + 1))?;
            db.execute_batch("COMMIT")?;
            Ok(())
        })();

        if let Err(err) = result {
            let _ = db.execute_batch("ROLLBACK");
            return Err(err);
        }
    }

    Ok(())
}

fn has_column(db: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    Ok(db
        .prepare(&format!("PRAGMA table_info({table})"))?
        .query_map([], |row| row.get::<_, String>(1))?
        .into_iter()
        .any(|name| name.is_ok_and(|n| n == column)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn user_version(db: &Connection) -> rusqlite::Result<i64> {
        db.query_row("PRAGMA user_version", [], |row| row.get(0))
    }

    #[test]
    fn a_new_database_ends_at_the_last_version() {
        let db = Connection::open_in_memory().unwrap();
        migrate(&db).unwrap();
        assert_eq!(user_version(&db).unwrap(), STEPS.len() as i64);
    }

    #[test]
    fn migrating_twice_changes_nothing() {
        let db = Connection::open_in_memory().unwrap();
        migrate(&db).unwrap();
        let first = user_version(&db).unwrap();
        migrate(&db).unwrap();
        let second = user_version(&db).unwrap();
        assert_eq!(first, second);
        assert_eq!(second, STEPS.len() as i64);
    }

    #[test]
    fn a_pre_versioned_database_gets_its_missing_columns() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE projects (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 workdir TEXT NOT NULL,
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL
             );
             CREATE TABLE sessions (
                 id TEXT PRIMARY KEY,
                 project_id TEXT NOT NULL,
                 workdir TEXT NOT NULL,
                 title TEXT,
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL
             );",
        )
        .unwrap();

        migrate(&db).unwrap();

        assert!(has_column(&db, "projects", "favorite").unwrap());
        assert!(has_column(&db, "sessions", "parent_id").unwrap());
        assert!(has_column(&db, "sessions", "linked_project_id").unwrap());
        assert_eq!(user_version(&db).unwrap(), STEPS.len() as i64);
    }

    #[test]
    fn a_database_from_a_newer_fennec_is_refused() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(&format!("PRAGMA user_version = {};", STEPS.len() + 1))
            .unwrap();
        assert!(migrate(&db).is_err());
    }
}
