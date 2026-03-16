use serde::Serialize;
use std::env;
use std::fs;
use std::path::PathBuf;

pub fn home_dir() -> PathBuf {
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

pub fn default_todos_path() -> PathBuf {
    home_dir().join(".todos.json")
}

pub fn default_categories_path() -> PathBuf {
    home_dir().join(".todo_categories.json")
}

pub fn load_json<T: serde::de::DeserializeOwned + Default>(path: &PathBuf) -> Result<T, String> {
    if !path.exists() {
        return Ok(T::default());
    }
    let data = fs::read_to_string(path)
        .map_err(|e| format!("Error reading {}: {e}", path.display()))?;
    serde_json::from_str(&data)
        .map_err(|e| format!("Error parsing {}: {e}", path.display()))
}

pub fn save_json<T: Serialize + ?Sized>(path: &PathBuf, value: &T) -> Result<(), String> {
    let data = serde_json::to_string_pretty(value).map_err(|e| format!("serialize: {e}"))?;
    fs::write(path, data).map_err(|e| format!("Error writing {}: {e}", path.display()))
}

/// Holds the file paths used for persistence. Override in tests to use temp files.
#[derive(Clone)]
pub struct Store {
    pub todos_path: PathBuf,
    pub categories_path: PathBuf,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            todos_path: default_todos_path(),
            categories_path: default_categories_path(),
        }
    }
}

impl Store {
    /// Construct a Store rooted at an arbitrary directory. Useful in tests.
    pub fn from_dir(dir: &std::path::Path) -> Self {
        Self {
            todos_path: dir.join("todos.json"),
            categories_path: dir.join("categories.json"),
        }
    }
}
