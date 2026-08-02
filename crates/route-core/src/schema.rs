//! SQLite schema definition and migrations for Route basic mode.

use anyhow::{Context, Result};
use rusqlite::Connection;

/// Schema version stored in `meta` table.
pub type SchemaVersion = u32;

pub const CURRENT_SCHEMA_VERSION: SchemaVersion = 4;

/// Apply all migrations up to `CURRENT_SCHEMA_VERSION` on a fresh or existing DB.
pub fn migrate(conn: &Connection) -> Result<()> {
    let current: SchemaVersion = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if current >= CURRENT_SCHEMA_VERSION {
        return Ok(());
    }

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )?;

    let migrations: &[(SchemaVersion, &str)] = &[
        (1, include_str!("../migrations/v001_basic.sql")),
        (2, include_str!("../migrations/v002_tags.sql")),
        (3, include_str!("../migrations/v003_operator.sql")),
        (4, include_str!("../migrations/v004_ai_conflicts.sql")),
        // future migrations appended here
    ];

    let mut applied = current;
    for (version, sql) in migrations {
        if *version > applied {
            conn.execute_batch(sql)
                .with_context(|| format!("applying migration v{version}"))?;
            applied = *version;
        }
    }

    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', ?1)",
        [applied.to_string()],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_creates_schema() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let v: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(v, "4");
    }

    #[test]
    fn migrate_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
    }
}
