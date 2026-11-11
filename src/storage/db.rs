use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use rusqlite::Connection;

const MIGRATION_V1: &str = include_str!("migrations/001_init.sql");

pub fn database_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("DRAWERAMA_DB") {
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("could not create {}", parent.display()))?;
            }
        }
        return Ok(path);
    }
    default_db_path()
}

pub fn default_db_path() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("com", "", "drawerama")
        .ok_or_else(|| anyhow!("could not resolve a data directory"))?;
    let dir = dirs.data_dir();
    std::fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;
    Ok(dir.join("drawerama.db"))
}

pub fn open(path: &std::path::Path) -> Result<Connection> {
    let conn =
        Connection::open(path).with_context(|| format!("could not open {}", path.display()))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .context("could not enable foreign keys")?;
    conn.busy_timeout(std::time::Duration::from_secs(2))?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .context("could not read the schema version")?;
    if version < 1 {
        conn.execute_batch(MIGRATION_V1)
            .context("could not create the database schema")?;
        conn.execute_batch("PRAGMA user_version = 1")
            .context("could not store the schema version")?;
    }
    Ok(())
}
