use crate::auth::{cmd_login, cmd_logout, cmd_register, require_auth};
use crate::category::{parse_category, Category, BUILTIN_CATEGORIES};
use crate::storage::Store;
use crate::todo::{
    clear_category_from_todos, delete_todo, insert_todo, load_custom_categories, load_todos,
    rename_category_in_todos, save_custom_categories, save_todos, Todo,
};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::Print,
    terminal::{self, ClearType},
};
use std::io::{stdout, Write};

pub fn cmd_add(
    store: &Store,
    username: &str,
    text: &str,
    category: Option<Category>,
) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("Todo text cannot be empty.".to_string());
    }
    let conn = store.open()?;
    let todo = insert_todo(&conn, text.to_string(), category, Some(username.to_string()))?;
    let id = todo.id;
    match todo.category {
        Some(ref cat) => println!("Added todo #{id} [{cat}]: {text}"),
        None => println!("Added todo #{id}: {text}"),
    }
    Ok(())
}

pub const PAGE_SIZE: usize = 10;

pub fn cmd_list(store: &Store, username: &str, page: usize) -> Result<(), String> {
    if page == 0 {
        return Err("Page number must be 1 or greater.".to_string());
    }
    let conn = store.open()?;
    let todos = load_todos(&conn, username)?;
    if todos.is_empty() {
        println!("No todos.");
        return Ok(());
    }
    let total_pages = todos.len().div_ceil(PAGE_SIZE);
    if page > total_pages {
        return Err(format!(
            "Page {page} out of range (total pages: {total_pages})."
        ));
    }
    let start = (page - 1) * PAGE_SIZE;
    let end = (start + PAGE_SIZE).min(todos.len());
    for t in &todos[start..end] {
        let mark = if t.done { "x" } else { " " };
        let cat_label = match &t.category {
            Some(c) => format!(" [{c}]"),
            None => String::new(),
        };
        println!("[{mark}] #{}{}: {}", t.id, cat_label, t.text);
    }
    if total_pages > 1 {
        println!("Page {page}/{total_pages}");
    }
    Ok(())
}

/// Render one page of todos into the already-cleared terminal area.
fn render_page(todos: &[Todo], page: usize, total_pages: usize) -> Result<(), String> {
    let start = (page - 1) * PAGE_SIZE;
    let end = (start + PAGE_SIZE).min(todos.len());
    let mut out = stdout();
    // Move to top-left and clear from cursor down so stale lines are erased.
    queue!(out, cursor::MoveTo(0, 0), terminal::Clear(ClearType::FromCursorDown))
        .map_err(|e| format!("render: {e}"))?;
    for t in &todos[start..end] {
        let mark = if t.done { "x" } else { " " };
        let cat_label = match &t.category {
            Some(c) => format!(" [{c}]"),
            None => String::new(),
        };
        queue!(out, Print(format!("[{mark}] #{}{}: {}\r\n", t.id, cat_label, t.text)))
            .map_err(|e| format!("render: {e}"))?;
    }
    queue!(
        out,
        Print(format!(
            "\r\nPage {page}/{total_pages}  \u{2190}/\u{2192} navigate  q quit\r\n"
        ))
    )
    .map_err(|e| format!("render: {e}"))?;
    out.flush().map_err(|e| format!("flush: {e}"))
}

/// Interactive pager: left/right arrows navigate pages, q/Esc/Enter exits.
pub fn cmd_list_interactive(store: &Store, username: &str) -> Result<(), String> {
    let conn = store.open()?;
    let todos = load_todos(&conn, username)?;
    if todos.is_empty() {
        println!("No todos.");
        return Ok(());
    }
    let total_pages = todos.len().div_ceil(PAGE_SIZE);
    // Non-interactive: skip raw mode when there is only one page.
    if total_pages == 1 {
        return cmd_list(store, username, 1);
    }

    let mut page: usize = 1;
    let mut out = stdout();
    terminal::enable_raw_mode().map_err(|e| format!("enable raw mode: {e}"))?;
    // Hide cursor while paging to reduce flicker.
    let _ = execute!(out, cursor::Hide);

    let result = render_page(&todos, page, total_pages).and_then(|()| {
        loop {
            match event::read() {
                Ok(Event::Key(KeyEvent { code, modifiers, .. })) => match code {
                    KeyCode::Right | KeyCode::Char('l') => {
                        if page < total_pages {
                            page += 1;
                            if let Err(e) = render_page(&todos, page, total_pages) {
                                break Err(e);
                            }
                        }
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        if page > 1 {
                            page -= 1;
                            if let Err(e) = render_page(&todos, page, total_pages) {
                                break Err(e);
                            }
                        }
                    }
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        break Ok(());
                    }
                    KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => {
                        break Ok(());
                    }
                    _ => {}
                },
                Ok(_) => {}
                Err(e) => break Err(format!("read event: {e}")),
            }
        }
    });

    // Always restore terminal state. If cleanup fails, prefer the loop result
    // so the caller sees the original error rather than a secondary one.
    let _ = execute!(out, cursor::Show, terminal::Clear(ClearType::FromCursorDown));
    let cleanup = terminal::disable_raw_mode().map_err(|e| format!("disable raw mode: {e}"));
    result.or(cleanup)
}

pub fn cmd_done(store: &Store, username: &str, id: u32) -> Result<(), String> {
    let conn = store.open()?;
    let mut todos = load_todos(&conn, username)?;
    if let Some(t) = todos.iter_mut().find(|t| t.id == id) {
        if t.owner.as_deref() != Some(username) {
            return Err(format!("Todo #{id} is not owned by you."));
        }
        if t.done {
            return Err(format!("Todo #{id} is already done."));
        }
        t.done = true;
        save_todos(&conn, &todos)?;
        println!("Marked #{id} as done.");
        Ok(())
    } else {
        Err(format!("Todo #{id} not found."))
    }
}

pub fn cmd_remove(store: &Store, username: &str, id: u32) -> Result<(), String> {
    let conn = store.open()?;
    let todos = load_todos(&conn, username)?;
    let todo = todos.iter().find(|t| t.id == id)
        .ok_or_else(|| format!("Todo #{id} not found."))?;
    if todo.owner.as_deref() != Some(username) {
        return Err(format!("Todo #{id} is not owned by you."));
    }
    delete_todo(&conn, id)?;
    println!("Removed #{id}.");
    Ok(())
}

pub fn cmd_edit(store: &Store, username: &str, id: u32, new_text: &str) -> Result<(), String> {
    if new_text.trim().is_empty() {
        return Err("Todo text cannot be empty.".to_string());
    }
    let conn = store.open()?;
    let mut todos = load_todos(&conn, username)?;
    if let Some(t) = todos.iter_mut().find(|t| t.id == id) {
        if t.owner.as_deref() != Some(username) {
            return Err(format!("Todo #{id} is not owned by you."));
        }
        t.text = new_text.to_string();
        save_todos(&conn, &todos)?;
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

pub fn cmd_category_remove(store: &Store, name: &str) -> Result<(), String> {
    let lower = name.to_lowercase();
    if BUILTIN_CATEGORIES.contains(&lower.as_str()) {
        return Err(format!("'{name}' is a built-in category and cannot be removed."));
    }
    let mut cats = load_custom_categories(store)?;
    let len = cats.len();
    cats.retain(|c| c.to_lowercase() != lower);
    if cats.len() == len {
        return Err(format!("Category '{name}' not found."));
    }
    save_custom_categories(store, &cats)?;
    clear_category_from_todos(store, name)?;
    println!("Removed category: {name}");
    Ok(())
}

pub fn cmd_category_edit(store: &Store, old_name: &str, new_name: &str) -> Result<(), String> {
    let old_lower = old_name.to_lowercase();
    let new_lower = new_name.to_lowercase();

    if BUILTIN_CATEGORIES.contains(&old_lower.as_str()) {
        return Err(format!("'{old_name}' is a built-in category and cannot be renamed."));
    }
    if new_name.trim().is_empty() {
        return Err("Category name cannot be empty.".to_string());
    }
    if BUILTIN_CATEGORIES.contains(&new_lower.as_str()) {
        return Err(format!("'{new_name}' is a built-in category name."));
    }

    let mut cats = load_custom_categories(store)?;

    let pos = cats
        .iter()
        .position(|c| c.to_lowercase() == old_lower)
        .ok_or_else(|| format!("Category '{old_name}' not found."))?;

    if cats.iter().any(|c| c.to_lowercase() == new_lower) {
        return Err(format!("Category '{new_name}' already exists."));
    }

    cats[pos] = new_name.to_string();
    save_custom_categories(store, &cats)?;
    rename_category_in_todos(store, old_name, new_name)?;
    println!("Renamed category '{old_name}' to '{new_name}'.");
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
    eprintln!("Auth commands:");
    eprintln!("  register <username> <password>        Create a new account and log in");
    eprintln!("  login <username> <password>           Log in to an existing account");
    eprintln!("  logout                                End the current session");
    eprintln!();
    eprintln!("Todo commands (require login):");
    eprintln!("  add [--cat <category>] <text>         Add a new todo");
    eprintln!("  list [--page <n>]                     List todos (10 per page)");
    eprintln!("  done <id>                             Mark a todo as done");
    eprintln!("  edit <id> <new text>                  Update the text of a todo");
    eprintln!("  remove <id>                           Remove a todo");
    eprintln!("  category add <name>                   Add a custom category");
    eprintln!("  category edit <name> <new name>       Rename a custom category");
    eprintln!("  category remove <name>                Remove a custom category");
    eprintln!("  category list                         List all categories");
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

    // Auth commands do not require an active session.
    match argv.as_slice() {
        ["register", username, password] => return cmd_register(store, username, password),
        ["register", ..] => {
            return Err("Usage: todo register <username> <password>".to_string())
        }
        ["login", username, password] => return cmd_login(store, username, password),
        ["login", ..] => return Err("Usage: todo login <username> <password>".to_string()),
        ["logout"] => return cmd_logout(),
        _ => {}
    }

    // All remaining commands require an active session.
    let username = require_auth()?;

    match argv.as_slice() {
        ["add", "--cat", cat, rest @ ..] if !rest.is_empty() => {
            let custom_cats = load_custom_categories(store)?;
            let category = parse_category(cat, &custom_cats).map_err(|e| {
                format!("{e}\nUse 'todo category list' to see available categories.")
            })?;
            cmd_add(store, &username, &rest.join(" "), Some(category))
        }
        ["add", "--cat", ..] => Err("Usage: todo add --cat <category> <text>".to_string()),
        ["add", rest @ ..] if !rest.is_empty() => cmd_add(store, &username, &rest.join(" "), None),
        ["add"] => Err("Usage: todo add [--cat <category>] <text>".to_string()),
        ["list"] => cmd_list_interactive(store, &username),
        ["list", "--page", page_str] => {
            let page: usize = page_str
                .parse()
                .map_err(|_| format!("Invalid page number: {page_str}"))?;
            cmd_list(store, &username, page)
        }
        ["list", ..] => Err("Usage: todo list [--page <n>]".to_string()),
        ["done", id_str] => cmd_done(store, &username, parse_id(id_str)?),
        ["done", ..] => Err("Usage: todo done <id>".to_string()),
        ["edit", id_str, rest @ ..] if !rest.is_empty() => {
            cmd_edit(store, &username, parse_id(id_str)?, &rest.join(" "))
        }
        ["edit", ..] => Err("Usage: todo edit <id> <new text>".to_string()),
        ["remove", id_str] => cmd_remove(store, &username, parse_id(id_str)?),
        ["remove", ..] => Err("Usage: todo remove <id>".to_string()),
        ["category", "add", name] => cmd_category_add(store, name),
        ["category", "add", ..] => Err("Usage: todo category add <name>".to_string()),
        ["category", "edit", old_name, new_name] => cmd_category_edit(store, old_name, new_name),
        ["category", "edit", ..] => {
            Err("Usage: todo category edit <name> <new name>".to_string())
        }
        ["category", "remove", name] => cmd_category_remove(store, name),
        ["category", "remove", ..] => Err("Usage: todo category remove <name>".to_string()),
        ["category", "list"] => cmd_category_list(store),
        ["category", sub, ..] => Err(format!("Unknown subcommand: category {sub}")),
        ["category"] => Err("Usage: todo category <add|edit|remove|list>".to_string()),
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
