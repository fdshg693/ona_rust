use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::process;

// --- Category ---

const BUILTIN_CATEGORIES: &[&str] = &["work", "personal", "shopping", "health"];

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Category {
    Work,
    Personal,
    Shopping,
    Health,
    Custom(String),
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Category::Work => write!(f, "work"),
            Category::Personal => write!(f, "personal"),
            Category::Shopping => write!(f, "shopping"),
            Category::Health => write!(f, "health"),
            Category::Custom(name) => write!(f, "{name}"),
        }
    }
}

/// Parse a category name. Accepts built-in names and registered custom categories.
fn parse_category(name: &str, custom_categories: &[String]) -> Result<Category, String> {
    let lower = name.to_lowercase();
    match lower.as_str() {
        "work" => Ok(Category::Work),
        "personal" => Ok(Category::Personal),
        "shopping" => Ok(Category::Shopping),
        "health" => Ok(Category::Health),
        _ => {
            if custom_categories.iter().any(|c| c.to_lowercase() == lower) {
                // Preserve the casing stored in the custom categories list
                let stored = custom_categories
                    .iter()
                    .find(|c| c.to_lowercase() == lower)
                    .unwrap();
                Ok(Category::Custom(stored.clone()))
            } else {
                Err(format!("Unknown category: {name}"))
            }
        }
    }
}

// --- Data paths ---

fn home_dir() -> PathBuf {
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn todos_path() -> PathBuf {
    home_dir().join(".todos.json")
}

fn categories_path() -> PathBuf {
    home_dir().join(".todo_categories.json")
}

// --- Persistence helpers ---

fn load_json<T: serde::de::DeserializeOwned>(path: &PathBuf) -> T
where
    T: Default,
{
    if !path.exists() {
        return T::default();
    }
    let data = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Error reading {}: {e}", path.display());
        process::exit(1);
    });
    serde_json::from_str(&data).unwrap_or_else(|e| {
        eprintln!("Error parsing {}: {e}", path.display());
        process::exit(1);
    })
}

fn save_json<T: Serialize>(path: &PathBuf, value: &T) {
    let data = serde_json::to_string_pretty(value).expect("serialize");
    fs::write(path, data).unwrap_or_else(|e| {
        eprintln!("Error writing {}: {e}", path.display());
        process::exit(1);
    });
}

// --- Todo ---

#[derive(Serialize, Deserialize, Clone)]
struct Todo {
    id: u32,
    text: String,
    done: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    category: Option<Category>,
}

fn load_todos() -> Vec<Todo> {
    load_json(&todos_path())
}

fn save_todos(todos: &[Todo]) {
    save_json(&todos_path(), &todos.to_vec());
}

fn load_custom_categories() -> Vec<String> {
    load_json(&categories_path())
}

fn save_custom_categories(cats: &[String]) {
    save_json(&categories_path(), &cats.to_vec());
}

fn next_id(todos: &[Todo]) -> u32 {
    todos.iter().map(|t| t.id).max().unwrap_or(0) + 1
}

// --- Commands ---

fn cmd_add(text: &str, category: Option<Category>) {
    let mut todos = load_todos();
    let id = next_id(&todos);
    todos.push(Todo {
        id,
        text: text.to_string(),
        done: false,
        category: category.clone(),
    });
    save_todos(&todos);
    match category {
        Some(cat) => println!("Added todo #{id} [{cat}]: {text}"),
        None => println!("Added todo #{id}: {text}"),
    }
}

fn cmd_list() {
    let todos = load_todos();
    if todos.is_empty() {
        println!("No todos.");
        return;
    }
    for t in &todos {
        let mark = if t.done { "x" } else { " " };
        let cat_label = match &t.category {
            Some(c) => format!(" [{c}]"),
            None => String::new(),
        };
        println!("[{mark}] #{}{}: {}", t.id, cat_label, t.text);
    }
}

fn cmd_done(id: u32) {
    let mut todos = load_todos();
    if let Some(t) = todos.iter_mut().find(|t| t.id == id) {
        t.done = true;
        save_todos(&todos);
        println!("Marked #{id} as done.");
    } else {
        eprintln!("Todo #{id} not found.");
        process::exit(1);
    }
}

fn cmd_remove(id: u32) {
    let mut todos = load_todos();
    let len = todos.len();
    todos.retain(|t| t.id != id);
    if todos.len() == len {
        eprintln!("Todo #{id} not found.");
        process::exit(1);
    }
    save_todos(&todos);
    println!("Removed #{id}.");
}

fn cmd_category_add(name: &str) {
    let lower = name.to_lowercase();
    if BUILTIN_CATEGORIES.contains(&lower.as_str()) {
        eprintln!("'{name}' is a built-in category.");
        process::exit(1);
    }
    let mut cats = load_custom_categories();
    if cats.iter().any(|c| c.to_lowercase() == lower) {
        eprintln!("Category '{name}' already exists.");
        process::exit(1);
    }
    cats.push(name.to_string());
    save_custom_categories(&cats);
    println!("Added category: {name}");
}

fn cmd_category_list() {
    println!("Built-in:");
    for c in BUILTIN_CATEGORIES {
        println!("  {c}");
    }
    let cats = load_custom_categories();
    if !cats.is_empty() {
        println!("Custom:");
        for c in &cats {
            println!("  {c}");
        }
    }
}

fn print_usage() {
    eprintln!("Usage: todo <command> [args]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  add [--cat <category>] <text>   Add a new todo");
    eprintln!("  list                             List all todos");
    eprintln!("  done <id>                        Mark a todo as done");
    eprintln!("  remove <id>                      Remove a todo");
    eprintln!("  category add <name>              Add a custom category");
    eprintln!("  category list                    List all categories");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    match args[1].as_str() {
        "add" => {
            if args.len() < 3 {
                eprintln!("Usage: todo add [--cat <category>] <text>");
                process::exit(1);
            }
            // Parse optional --cat flag
            if args[2] == "--cat" {
                if args.len() < 5 {
                    eprintln!("Usage: todo add --cat <category> <text>");
                    process::exit(1);
                }
                let custom_cats = load_custom_categories();
                let category = parse_category(&args[3], &custom_cats).unwrap_or_else(|e| {
                    eprintln!("{e}");
                    eprintln!("Use 'todo category list' to see available categories.");
                    process::exit(1);
                });
                let text = args[4..].join(" ");
                cmd_add(&text, Some(category));
            } else {
                let text = args[2..].join(" ");
                cmd_add(&text, None);
            }
        }
        "list" => cmd_list(),
        "done" => {
            if args.len() < 3 {
                eprintln!("Usage: todo done <id>");
                process::exit(1);
            }
            let id: u32 = args[2].parse().unwrap_or_else(|_| {
                eprintln!("Invalid id: {}", args[2]);
                process::exit(1);
            });
            cmd_done(id);
        }
        "remove" => {
            if args.len() < 3 {
                eprintln!("Usage: todo remove <id>");
                process::exit(1);
            }
            let id: u32 = args[2].parse().unwrap_or_else(|_| {
                eprintln!("Invalid id: {}", args[2]);
                process::exit(1);
            });
            cmd_remove(id);
        }
        "category" => {
            if args.len() < 3 {
                eprintln!("Usage: todo category <add|list>");
                process::exit(1);
            }
            match args[2].as_str() {
                "add" => {
                    if args.len() < 4 {
                        eprintln!("Usage: todo category add <name>");
                        process::exit(1);
                    }
                    cmd_category_add(&args[3]);
                }
                "list" => cmd_category_list(),
                _ => {
                    eprintln!("Unknown subcommand: category {}", args[2]);
                    process::exit(1);
                }
            }
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage();
            process::exit(1);
        }
    }
}
