use crate::storage::Store;
use bcrypt::{hash, verify, DEFAULT_COST};
use rusqlite::OptionalExtension;
use std::cell::RefCell;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;


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

// ── Shared DB helpers (used by both CLI and REST) ─────────────────────────────

/// Insert a new user row. Returns `Err` with a user-facing message on duplicate username
/// (case-insensitive check) or any other DB failure.
pub fn db_create_user(store: &Store, username: &str, password: &str) -> Result<(), String> {
    let conn = store.open()?;

    // Case-insensitive duplicate check — SQLite's UNIQUE constraint is case-sensitive
    // by default, so "Alice" and "alice" would both succeed without this guard.
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM users WHERE LOWER(username) = LOWER(?1)",
            rusqlite::params![username],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|_| "Database error.".to_string())?
        > 0;

    if exists {
        return Err(format!("Username '{username}' is already taken."));
    }

    let password_hash =
        hash(password, DEFAULT_COST).map_err(|_| "Failed to hash password.".to_string())?;

    conn.execute(
        "INSERT INTO users (username, password_hash) VALUES (?1, ?2)",
        rusqlite::params![username, password_hash],
    )
    .map_err(|_| "Failed to create user.".to_string())?;
    Ok(())
}

/// Verify credentials and return the canonical username on success.
/// Returns a generic error message on wrong password or unknown user.
pub fn db_verify_credentials(store: &Store, username: &str, password: &str) -> Result<String, String> {
    let conn = store.open()?;
    let result: Option<(String, String)> = conn
        .query_row(
            "SELECT username, password_hash FROM users WHERE LOWER(username) = LOWER(?1)",
            rusqlite::params![username],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| "Database error.".to_string())?;

    let (canonical, stored_hash) =
        result.ok_or_else(|| "Invalid username or password.".to_string())?;

    let valid = verify(password, &stored_hash)
        .map_err(|_| "Failed to verify password.".to_string())?;

    if !valid {
        return Err("Invalid username or password.".to_string());
    }
    Ok(canonical)
}

/// Register a new user. Fails if the username already exists.
pub fn cmd_register(store: &Store, username: &str, password: &str) -> Result<(), String> {
    if username.trim().is_empty() {
        return Err("Username cannot be empty.".to_string());
    }
    if password.is_empty() {
        return Err("Password cannot be empty.".to_string());
    }
    db_create_user(store, username, password)?;
    write_session(username)?;
    println!("Registered and logged in as '{username}'.");
    Ok(())
}

/// Verify credentials and write a session file on success.
pub fn cmd_login(store: &Store, username: &str, password: &str) -> Result<(), String> {
    if username.trim().is_empty() {
        return Err("Username cannot be empty.".to_string());
    }
    let canonical = db_verify_credentials(store, username, password)?;
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

// ── Token-based auth (REST API) ───────────────────────────────────────────────

/// How long a REST session token is valid (24 hours).
const TOKEN_TTL_SECS: u64 = 86_400;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Create a new opaque session token for `username`, persist it, and return it.
pub fn create_token(store: &Store, username: &str) -> Result<String, String> {
    let token = Uuid::new_v4().to_string();
    let expires_at = now_secs() + TOKEN_TTL_SECS;
    let conn = store.open()?;
    conn.execute(
        "INSERT INTO sessions (token, username, expires_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![token, username, expires_at as i64],
    )
    .map_err(|e| format!("Failed to create session token: {e}"))?;
    Ok(token)
}

/// Validate a token and return the associated username, or an error if invalid/expired.
pub fn validate_token(store: &Store, token: &str) -> Result<String, String> {
    let conn = store.open()?;
    let result: Option<(String, i64)> = conn
        .query_row(
            "SELECT username, expires_at FROM sessions WHERE token = ?1",
            rusqlite::params![token],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("DB error: {e}"))?;

    match result {
        None => Err("Invalid or expired token.".to_string()),
        Some((username, expires_at)) => {
            if now_secs() > expires_at as u64 {
                // Clean up expired token.
                let _ = conn.execute(
                    "DELETE FROM sessions WHERE token = ?1",
                    rusqlite::params![token],
                );
                Err("Invalid or expired token.".to_string())
            } else {
                Ok(username)
            }
        }
    }
}

/// Revoke a specific token (REST logout).
pub fn revoke_token(store: &Store, token: &str) -> Result<(), String> {
    let conn = store.open()?;
    conn.execute(
        "DELETE FROM sessions WHERE token = ?1",
        rusqlite::params![token],
    )
    .map_err(|e| format!("Failed to revoke token: {e}"))?;
    Ok(())
}
