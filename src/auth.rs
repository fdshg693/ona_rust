use crate::storage::Store;
use bcrypt::{hash, verify, DEFAULT_COST};
use rusqlite::OptionalExtension;
use std::cell::RefCell;
use std::env;
use std::fs;
use std::path::PathBuf;

// ── Session file ─────────────────────────────────────────────────────────────

thread_local! {
    /// Per-thread session path override. Used in tests to avoid touching ~/.todo_session
    /// and to prevent parallel tests from interfering with each other via a shared env var.
    static SESSION_PATH_OVERRIDE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Override the session file path for the current thread. Used in tests.
pub fn set_session_path_for_test(path: PathBuf) {
    SESSION_PATH_OVERRIDE.with(|o| *o.borrow_mut() = Some(path));
}

/// Returns the path to the session file (~/.todo_session).
/// A thread-local override (set via `set_session_path_for_test`) takes precedence.
pub fn session_path() -> Result<PathBuf, String> {
    let override_path = SESSION_PATH_OVERRIDE.with(|o| o.borrow().clone());
    if let Some(p) = override_path {
        return Ok(p);
    }
    env::var("HOME")
        .map(|h| PathBuf::from(h).join(".todo_session"))
        .map_err(|_| "HOME environment variable is not set".to_string())
}

/// Write the logged-in username to the session file.
pub fn write_session(username: &str) -> Result<(), String> {
    let path = session_path()?;
    fs::write(&path, username).map_err(|e| format!("Failed to write session: {e}"))
}

/// Read the current session username, or return an error if not logged in.
pub fn read_session() -> Result<String, String> {
    let path = session_path()?;
    fs::read_to_string(&path)
        .map(|s| s.trim().to_string())
        .map_err(|_| {
            "Not logged in. Run 'todo login <username>' or 'todo register <username>'.".to_string()
        })
        .and_then(|s| {
            if s.is_empty() {
                Err("Not logged in. Run 'todo login <username>' or 'todo register <username>'.".to_string())
            } else {
                Ok(s)
            }
        })
}

/// Delete the session file (logout).
pub fn clear_session() -> Result<(), String> {
    let path = session_path()?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to remove session: {e}"))?;
    }
    Ok(())
}

// ── User management ──────────────────────────────────────────────────────────

/// Register a new user. Fails if the username already exists.
pub fn cmd_register(store: &Store, username: &str, password: &str) -> Result<(), String> {
    if username.trim().is_empty() {
        return Err("Username cannot be empty.".to_string());
    }
    if password.is_empty() {
        return Err("Password cannot be empty.".to_string());
    }

    let conn = store.open()?;

    // Check for duplicate username (case-insensitive).
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM users WHERE LOWER(username) = LOWER(?1)",
            rusqlite::params![username],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("DB error: {e}"))?
        > 0;

    if exists {
        return Err(format!("Username '{username}' is already taken."));
    }

    let password_hash =
        hash(password, DEFAULT_COST).map_err(|e| format!("Failed to hash password: {e}"))?;

    conn.execute(
        "INSERT INTO users (username, password_hash) VALUES (?1, ?2)",
        rusqlite::params![username, password_hash],
    )
    .map_err(|e| format!("Failed to create user: {e}"))?;

    write_session(username)?;
    println!("Registered and logged in as '{username}'.");
    Ok(())
}

/// Verify credentials and write a session file on success.
pub fn cmd_login(store: &Store, username: &str, password: &str) -> Result<(), String> {
    if username.trim().is_empty() {
        return Err("Username cannot be empty.".to_string());
    }

    let conn = store.open()?;

    let result: Option<String> = conn
        .query_row(
            "SELECT password_hash FROM users WHERE LOWER(username) = LOWER(?1)",
            rusqlite::params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("DB error: {e}"))?;

    let stored_hash = result.ok_or_else(|| "Invalid username or password.".to_string())?;

    let valid =
        verify(password, &stored_hash).map_err(|e| format!("Failed to verify password: {e}"))?;

    if !valid {
        return Err("Invalid username or password.".to_string());
    }

    // Store the canonical casing from the DB.
    let canonical: String = conn
        .query_row(
            "SELECT username FROM users WHERE LOWER(username) = LOWER(?1)",
            rusqlite::params![username],
            |row| row.get(0),
        )
        .map_err(|e| format!("DB error: {e}"))?;

    write_session(&canonical)?;
    println!("Logged in as '{canonical}'.");
    Ok(())
}

/// Remove the session file.
pub fn cmd_logout() -> Result<(), String> {
    clear_session()?;
    println!("Logged out.");
    Ok(())
}

/// Return the currently logged-in username, or an error if not authenticated.
pub fn require_auth() -> Result<String, String> {
    read_session()
}
