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

fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS todos (
            id       INTEGER PRIMARY KEY,
            text     TEXT    NOT NULL,
            done     INTEGER NOT NULL DEFAULT 0,
            category TEXT
        );
        CREATE TABLE IF NOT EXISTS categories (
            name TEXT PRIMARY KEY
        );",
    )
    .map_err(|e| format!("Failed to initialise schema: {e}"))
}
