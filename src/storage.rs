use rusqlite::Connection;
use std::env;
use std::path::PathBuf;

fn home_dir() -> Result<PathBuf, String> {
    env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| "HOME environment variable is not set".to_string())
}

/// Holds the path to the SQLite database file.
#[derive(Clone)]
pub struct Store {
    pub db_path: PathBuf,
}

impl Store {
    /// Construct a Store using the user's home directory. Returns an error if HOME is unset.
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            db_path: home_dir()?.join(".todos.db"),
        })
    }

    /// Construct a Store rooted at an arbitrary directory. Useful in tests.
    pub fn from_dir(dir: &std::path::Path) -> Self {
        Self {
            db_path: dir.join("todos.db"),
        }
    }

    /// Open a connection and ensure both tables exist.
    pub fn open(&self) -> Result<Connection, String> {
        let conn = Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open database: {e}"))?;
        init_schema(&conn)?;
        Ok(conn)
    }
}

/// Current schema version. Increment this whenever a migration is added.
const SCHEMA_VERSION: u32 = 2;

fn init_schema(conn: &Connection) -> Result<(), String> {
    // Create base tables (version 1).
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS users (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            username      TEXT    NOT NULL UNIQUE,
            password_hash TEXT    NOT NULL
        );
        CREATE TABLE IF NOT EXISTS todos (
            id       INTEGER PRIMARY KEY,
            text     TEXT    NOT NULL,
            done     INTEGER NOT NULL DEFAULT 0,
            category TEXT,
            owner    TEXT
        );
        CREATE TABLE IF NOT EXISTS categories (
            name TEXT PRIMARY KEY
        );
        CREATE TABLE IF NOT EXISTS sessions (
            token      TEXT    PRIMARY KEY,
            username   TEXT    NOT NULL,
            expires_at INTEGER NOT NULL
        );",
    )
    .map_err(|e| format!("Failed to initialise schema: {e}"))?;

    let version: u32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| format!("Failed to read schema version: {e}"))?;

    if version < 2 {
        if version == 0 {
            // version 0 means either a brand-new DB or a pre-versioning DB.
            // Check whether the owner column already exists (new DB) before
            // attempting the ALTER TABLE (pre-versioning DB).
            let has_owner: bool = conn
                .prepare("SELECT owner FROM todos LIMIT 0")
                .is_ok();
            if !has_owner {
                // Pre-versioning DB: owner column is absent, add it.
                conn.execute_batch("ALTER TABLE todos ADD COLUMN owner TEXT")
                    .map_err(|e| format!("Migration v2 failed: {e}"))?;
            }
        }
        conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
            .map_err(|e| format!("Failed to set schema version: {e}"))?;
    }

    Ok(())
}
