use crate::category::{parse_category, Category, BUILTIN_CATEGORIES};
use crate::storage::Store;
use crate::todo::{
    load_custom_categories, load_todos, next_id, save_custom_categories, save_todos, Todo,
};

pub fn cmd_add(store: &Store, text: &str, category: Option<Category>) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("Todo text cannot be empty.".to_string());
    }
    let mut todos = load_todos(store)?;
    let id = next_id(&todos)?;
    todos.push(Todo {
        id,
        text: text.to_string(),
        done: false,
        category: category.clone(),
    });
    save_todos(store, &todos)?;
    match category {
        Some(cat) => println!("Added todo #{id} [{cat}]: {text}"),
        None => println!("Added todo #{id}: {text}"),
    }
    Ok(())
}

pub fn cmd_list(store: &Store) -> Result<(), String> {
    let todos = load_todos(store)?;
    if todos.is_empty() {
        println!("No todos.");
        return Ok(());
    }
    for t in &todos {
        let mark = if t.done { "x" } else { " " };
        let cat_label = match &t.category {
            Some(c) => format!(" [{c}]"),
            None => String::new(),
        };
        println!("[{mark}] #{}{}: {}", t.id, cat_label, t.text);
    }
    Ok(())
}

pub fn cmd_done(store: &Store, id: u32) -> Result<(), String> {
    let mut todos = load_todos(store)?;
    if let Some(t) = todos.iter_mut().find(|t| t.id == id) {
        if t.done {
            return Err(format!("Todo #{id} is already done."));
        }
        t.done = true;
        save_todos(store, &todos)?;
        println!("Marked #{id} as done.");
        Ok(())
    } else {
        Err(format!("Todo #{id} not found."))
    }
}

pub fn cmd_remove(store: &Store, id: u32) -> Result<(), String> {
    let mut todos = load_todos(store)?;
    let len = todos.len();
    todos.retain(|t| t.id != id);
    if todos.len() == len {
        return Err(format!("Todo #{id} not found."));
    }
    save_todos(store, &todos)?;
    println!("Removed #{id}.");
    Ok(())
}

pub fn cmd_edit(store: &Store, id: u32, new_text: &str) -> Result<(), String> {
    if new_text.trim().is_empty() {
        return Err("Todo text cannot be empty.".to_string());
    }
    let mut todos = load_todos(store)?;
    if let Some(t) = todos.iter_mut().find(|t| t.id == id) {
        t.text = new_text.to_string();
        save_todos(store, &todos)?;
        println!("Updated #{id}: {new_text}");
        Ok(())
    } else {
        Err(format!("Todo #{id} not found."))
    }
}

pub fn cmd_category_add(store: &Store, name: &str) -> Result<(), String> {
    let lower = name.to_lowercase();
    if BUILTIN_CATEGORIES.contains(&lower.as_str()) {
        return Err(format!("'{name}' is a built-in category."));
    }
    let mut cats = load_custom_categories(store)?;
    if cats.iter().any(|c| c.to_lowercase() == lower) {
        return Err(format!("Category '{name}' already exists."));
    }
    cats.push(name.to_string());
    save_custom_categories(store, &cats)?;
    println!("Added category: {name}");
    Ok(())
}

pub fn cmd_category_list(store: &Store) -> Result<(), String> {
    println!("Built-in:");
    for c in BUILTIN_CATEGORIES {
        println!("  {c}");
    }
    let cats = load_custom_categories(store)?;
    if !cats.is_empty() {
        println!("Custom:");
        for c in &cats {
            println!("  {c}");
        }
    }
    Ok(())
}

pub fn print_usage() {
    eprintln!("Usage: todo <command> [args]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  add [--cat <category>] <text>   Add a new todo");
    eprintln!("  list                             List all todos");
    eprintln!("  done <id>                        Mark a todo as done");
    eprintln!("  edit <id> <new text>             Update the text of a todo");
    eprintln!("  remove <id>                      Remove a todo");
    eprintln!("  category add <name>              Add a custom category");
    eprintln!("  category list                    List all categories");
}

fn parse_id(s: &str) -> Result<u32, String> {
    s.parse().map_err(|_| format!("Invalid id: {s}"))
}

pub fn run(args: &[String]) -> Result<(), String> {
    run_with_store(args, &Store::new()?)
}

pub fn run_with_store(args: &[String], store: &Store) -> Result<(), String> {
    // Skip argv[0] (program name) and match on the remaining arguments.
    let argv: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();

    match argv.as_slice() {
        ["add", "--cat", cat, rest @ ..] if !rest.is_empty() => {
            let custom_cats = load_custom_categories(store)?;
            let category = parse_category(cat, &custom_cats).map_err(|e| {
                format!("{e}\nUse 'todo category list' to see available categories.")
            })?;
            cmd_add(store, &rest.join(" "), Some(category))
        }
        ["add", "--cat", ..] => Err("Usage: todo add --cat <category> <text>".to_string()),
        ["add", rest @ ..] if !rest.is_empty() => cmd_add(store, &rest.join(" "), None),
        ["add"] => Err("Usage: todo add [--cat <category>] <text>".to_string()),
        ["list"] => cmd_list(store),
        ["done", id_str] => cmd_done(store, parse_id(id_str)?),
        ["done", ..] => Err("Usage: todo done <id>".to_string()),
        ["edit", id_str, rest @ ..] if !rest.is_empty() => {
            cmd_edit(store, parse_id(id_str)?, &rest.join(" "))
        }
        ["edit", ..] => Err("Usage: todo edit <id> <new text>".to_string()),
        ["remove", id_str] => cmd_remove(store, parse_id(id_str)?),
        ["remove", ..] => Err("Usage: todo remove <id>".to_string()),
        ["category", "add", name] => cmd_category_add(store, name),
        ["category", "add", ..] => Err("Usage: todo category add <name>".to_string()),
        ["category", "list"] => cmd_category_list(store),
        ["category", sub, ..] => Err(format!("Unknown subcommand: category {sub}")),
        ["category"] => Err("Usage: todo category <add|list>".to_string()),
        [cmd, ..] => {
            eprintln!("Unknown command: {cmd}");
            print_usage();
            Err(String::new())
        }
        [] => {
            print_usage();
            Err(String::new())
        }
    }
}
