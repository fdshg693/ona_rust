use crate::category::Category;
use crate::storage::Store;

#[derive(Clone)]
pub struct Todo {
    pub id: u32,
    pub text: String,
    pub done: bool,
    pub category: Option<Category>,
}

// Category is stored as its Display string; NULL means no category.
fn category_to_sql(cat: &Option<Category>) -> Option<String> {
    cat.as_ref().map(|c| c.to_string())
}

fn category_from_sql(s: Option<String>) -> Option<Category> {
    s.map(|name| match name.to_lowercase().as_str() {
        "work" => Category::Work,
        "personal" => Category::Personal,
        "shopping" => Category::Shopping,
        "health" => Category::Health,
        _ => Category::Custom(name),
    })
}

pub fn load_todos(store: &Store) -> Result<Vec<Todo>, String> {
    let conn = store.open()?;
    let mut stmt = conn
        .prepare("SELECT id, text, done, category FROM todos ORDER BY id")
        .map_err(|e| format!("prepare: {e}"))?;

    let todos = stmt
        .query_map([], |row| {
            Ok(Todo {
                id: row.get::<_, u32>(0)?,
                text: row.get::<_, String>(1)?,
                done: row.get::<_, bool>(2)?,
                category: {
                    let s: Option<String> = row.get(3)?;
                    Ok::<_, rusqlite::Error>(category_from_sql(s))
                }?,
            })
        })
        .map_err(|e| format!("query: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("row: {e}"))?;

    Ok(todos)
}

pub fn save_todos(store: &Store, todos: &[Todo]) -> Result<(), String> {
    let mut conn = store.open()?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("begin transaction: {e}"))?;
    tx.execute("DELETE FROM todos", [])
        .map_err(|e| format!("delete todos: {e}"))?;
    for t in todos {
        tx.execute(
            "INSERT INTO todos (id, text, done, category) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![t.id, t.text, t.done, category_to_sql(&t.category)],
        )
        .map_err(|e| format!("insert todo: {e}"))?;
    }
    tx.commit().map_err(|e| format!("commit: {e}"))
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
    let mut conn = store.open()?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("begin transaction: {e}"))?;
    tx.execute("DELETE FROM categories", [])
        .map_err(|e| format!("delete categories: {e}"))?;
    for name in cats {
        tx.execute(
            "INSERT INTO categories (name) VALUES (?1)",
            rusqlite::params![name],
        )
        .map_err(|e| format!("insert category: {e}"))?;
    }
    tx.commit().map_err(|e| format!("commit: {e}"))
}

pub fn next_id(todos: &[Todo]) -> Result<u32, String> {
    let max = todos.iter().map(|t| t.id).max().unwrap_or(0);
    max.checked_add(1)
        .ok_or_else(|| "todo id overflow: too many todos".to_string())
}
