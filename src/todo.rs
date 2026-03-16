use crate::category::Category;
use crate::storage::{load_json, save_json, Store};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Todo {
    pub id: u32,
    pub text: String,
    pub done: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<Category>,
}

pub fn load_todos(store: &Store) -> Result<Vec<Todo>, String> {
    load_json(&store.todos_path)
}

pub fn save_todos(store: &Store, todos: &[Todo]) -> Result<(), String> {
    save_json(&store.todos_path, todos)
}

pub fn load_custom_categories(store: &Store) -> Result<Vec<String>, String> {
    load_json(&store.categories_path)
}

pub fn save_custom_categories(store: &Store, cats: &[String]) -> Result<(), String> {
    save_json(&store.categories_path, cats)
}

pub fn next_id(todos: &[Todo]) -> Result<u32, String> {
    let max = todos.iter().map(|t| t.id).max().unwrap_or(0);
    max.checked_add(1)
        .ok_or_else(|| "todo id overflow: too many todos".to_string())
}
