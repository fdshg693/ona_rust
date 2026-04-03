use crate::category::{parse_category, Category};
use crate::storage::Store;
use rusqlite::{Connection, Transaction};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: u32,
    pub text: String,
    pub done: bool,
    pub category: Option<Category>,
    pub owner: Option<String>,
}

// Category is stored as its Display string; NULL means no category.
fn category_to_sql(cat: &Option<Category>) -> Option<String> {
    cat.as_ref().map(|c| c.to_string())
}

/// Run `body` inside a single transaction, committing on success.
fn with_transaction<F>(store: &Store, body: F) -> Result<(), String>
where
    F: FnOnce(&Transaction) -> Result<(), String>,
{
    let mut conn = store.open()?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("begin transaction: {e}"))?;
    body(&tx)?;
    tx.commit().map_err(|e| format!("commit: {e}"))
}

pub fn load_todos(conn: &Connection, username: &str) -> Result<Vec<Todo>, String> {
    let custom_cats: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT name FROM categories ORDER BY name")
            .map_err(|e| format!("prepare categories: {e}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| format!("query categories: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("row: {e}"))?
    };

    let mut stmt = conn
        .prepare(
            "SELECT id, text, done, category, owner FROM todos \
             WHERE owner IS NULL OR owner = ?1 ORDER BY id",
        )
        .map_err(|e| format!("prepare: {e}"))?;

    let mut rows = stmt
        .query(rusqlite::params![username])
        .map_err(|e| format!("query: {e}"))?;

    let mut todos = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|e| format!("row: {e}"))?
    {
        let id: u32 = row.get(0).map_err(|e| format!("row: {e}"))?;
        let text: String = row.get(1).map_err(|e| format!("row: {e}"))?;
        let done: bool = row.get(2).map_err(|e| format!("row: {e}"))?;
        let cat_opt: Option<String> = row.get(3).map_err(|e| format!("row: {e}"))?;
        let category = match cat_opt {
            None => None,
            Some(ref s) => Some(parse_category(s, &custom_cats)?),
        };
        let owner: Option<String> = row.get(4).map_err(|e| format!("row: {e}"))?;
        todos.push(Todo {
            id,
            text,
            done,
            category,
            owner,
        });
    }

    Ok(todos)
}

pub fn save_todos(conn: &Connection, todos: &[Todo]) -> Result<(), String> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("begin transaction: {e}"))?;
    for t in todos {
        tx.execute(
            "INSERT OR REPLACE INTO todos (id, text, done, category, owner) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                t.id,
                t.text,
                t.done,
                category_to_sql(&t.category),
                t.owner,
            ],
        )
        .map_err(|e| format!("upsert todo: {e}"))?;
    }
    tx.commit().map_err(|e| format!("commit: {e}"))
}

/// Remove a single todo row by primary key (ids are unique across the table).
pub fn delete_todo(conn: &Connection, id: u32) -> Result<(), String> {
    conn.execute("DELETE FROM todos WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| format!("delete todo: {e}"))?;
    Ok(())
}

pub fn load_custom_categories(store: &Store) -> Result<Vec<String>, String> {
    let conn = store.open()?;
    let mut stmt = conn
        .prepare("SELECT name FROM categories ORDER BY name")
        .map_err(|e| format!("prepare: {e}"))?;

    let cats = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("query: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("row: {e}"))?;

    Ok(cats)
}

pub fn save_custom_categories(store: &Store, cats: &[String]) -> Result<(), String> {
    let cats: Vec<String> = cats.to_vec();
    with_transaction(store, |tx| {
        tx.execute("DELETE FROM categories", [])
            .map_err(|e| format!("delete categories: {e}"))?;
        for name in &cats {
            tx.execute(
                "INSERT INTO categories (name) VALUES (?1)",
                rusqlite::params![name],
            )
            .map_err(|e| format!("insert category: {e}"))?;
        }
        Ok(())
    })
}

/// Set `category = NULL` on every todo whose category matches `name` (case-insensitive).
pub fn clear_category_from_todos(store: &Store, name: &str) -> Result<(), String> {
    let lower = name.to_lowercase();
    with_transaction(store, |tx| {
        tx.execute(
            "UPDATE todos SET category = NULL WHERE LOWER(category) = ?1",
            rusqlite::params![lower],
        )
        .map_err(|e| format!("clear category from todos: {e}"))?;
        Ok(())
    })
}

/// Replace the category string on every todo whose category matches `old_name` (case-insensitive).
pub fn rename_category_in_todos(store: &Store, old_name: &str, new_name: &str) -> Result<(), String> {
    let lower = old_name.to_lowercase();
    with_transaction(store, |tx| {
        tx.execute(
            "UPDATE todos SET category = ?1 WHERE LOWER(category) = ?2",
            rusqlite::params![new_name, lower],
        )
        .map_err(|e| format!("rename category in todos: {e}"))?;
        Ok(())
    })
}

/// Rename a custom category and update all referencing todos atomically.
///
/// `cats` is the already-updated categories list (old name replaced with new name).
pub fn rename_category_atomic(
    store: &Store,
    cats: &[String],
    old_name: &str,
    new_name: &str,
) -> Result<(), String> {
    let cats = cats.to_vec();
    let old_lower = old_name.to_lowercase();
    let new_name = new_name.to_string();
    with_transaction(store, |tx| {
        tx.execute("DELETE FROM categories", [])
            .map_err(|e| format!("delete categories: {e}"))?;
        for name in &cats {
            tx.execute(
                "INSERT INTO categories (name) VALUES (?1)",
                rusqlite::params![name],
            )
            .map_err(|e| format!("insert category: {e}"))?;
        }
        tx.execute(
            "UPDATE todos SET category = ?1 WHERE LOWER(category) = ?2",
            rusqlite::params![new_name, old_lower],
        )
        .map_err(|e| format!("rename category in todos: {e}"))?;
        Ok(())
    })
}

/// Remove a custom category and clear it from all referencing todos atomically.
///
/// `cats` is the already-filtered categories list (target name removed).
pub fn remove_category_atomic(store: &Store, cats: &[String], name: &str) -> Result<(), String> {
    let cats = cats.to_vec();
    let lower = name.to_lowercase();
    with_transaction(store, |tx| {
        tx.execute("DELETE FROM categories", [])
            .map_err(|e| format!("delete categories: {e}"))?;
        for cat in &cats {
            tx.execute(
                "INSERT INTO categories (name) VALUES (?1)",
                rusqlite::params![cat],
            )
            .map_err(|e| format!("insert category: {e}"))?;
        }
        tx.execute(
            "UPDATE todos SET category = NULL WHERE LOWER(category) = ?1",
            rusqlite::params![lower],
        )
        .map_err(|e| format!("clear category from todos: {e}"))?;
        Ok(())
    })
}

/// Return the next available todo ID (global max + 1).
///
/// Exposed for tests. Production code should use `insert_todo`, which
/// assigns the ID atomically inside a `BEGIN IMMEDIATE` transaction.
pub fn next_id(conn: &Connection) -> Result<u32, String> {
    let max: Option<u32> = conn
        .query_row("SELECT MAX(id) FROM todos", [], |row| row.get(0))
        .map_err(|e| format!("next_id: {e}"))?;
    max.unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "todo id overflow: too many todos".to_string())
}

/// Insert a new todo atomically, assigning the next available ID inside a
/// single `BEGIN IMMEDIATE` transaction to prevent concurrent writers from
/// claiming the same ID.
///
/// Returns the inserted `Todo` (with its assigned `id`).
pub fn insert_todo(conn: &Connection, text: String, category: Option<Category>, owner: Option<String>) -> Result<Todo, String> {
    // BEGIN IMMEDIATE acquires a write lock immediately, blocking other writers
    // for the duration of this transaction.
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| format!("begin immediate: {e}"))?;

    let result = (|| -> Result<Todo, String> {
        let max: Option<u32> = conn
            .query_row("SELECT MAX(id) FROM todos", [], |row| row.get(0))
            .map_err(|e| format!("next_id: {e}"))?;
        let id = max
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| "todo id overflow: too many todos".to_string())?;

        conn.execute(
            "INSERT INTO todos (id, text, done, category, owner) VALUES (?1, ?2, 0, ?3, ?4)",
            rusqlite::params![id, text, category_to_sql(&category), owner],
        )
        .map_err(|e| format!("insert todo: {e}"))?;

        Ok(Todo { id, text, done: false, category, owner })
    })();

    match result {
        Ok(todo) => {
            conn.execute_batch("COMMIT").map_err(|e| format!("commit: {e}"))?;
            Ok(todo)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}
