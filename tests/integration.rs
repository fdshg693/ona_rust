use ona_rust::category::{parse_category, Category};
use ona_rust::cli::run_with_store;
use ona_rust::storage::Store;
use ona_rust::todo::{load_custom_categories, load_todos, next_id};
use tempfile::TempDir;

fn args(parts: &[&str]) -> Vec<String> {
    // Prepend a fake argv[0] to match real CLI layout
    std::iter::once("todo")
        .chain(parts.iter().copied())
        .map(String::from)
        .collect()
}

fn temp_store() -> (Store, TempDir) {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path());
    (store, dir)
}

// ── category::parse_category ────────────────────────────────────────────────

#[test]
fn parse_builtin_categories() {
    let custom: Vec<String> = vec![];
    assert!(matches!(parse_category("work", &custom), Ok(Category::Work)));
    assert!(matches!(
        parse_category("personal", &custom),
        Ok(Category::Personal)
    ));
    assert!(matches!(
        parse_category("shopping", &custom),
        Ok(Category::Shopping)
    ));
    assert!(matches!(
        parse_category("health", &custom),
        Ok(Category::Health)
    ));
}

#[test]
fn parse_builtin_category_case_insensitive() {
    let custom: Vec<String> = vec![];
    assert!(matches!(parse_category("WORK", &custom), Ok(Category::Work)));
    assert!(matches!(
        parse_category("Health", &custom),
        Ok(Category::Health)
    ));
}

#[test]
fn parse_custom_category_found() {
    let custom = vec!["Hobby".to_string()];
    let result = parse_category("hobby", &custom);
    assert!(matches!(result, Ok(Category::Custom(ref s)) if s == "Hobby"));
}

#[test]
fn parse_unknown_category_returns_error() {
    let custom: Vec<String> = vec![];
    assert!(parse_category("unknown", &custom).is_err());
}

// ── todo::next_id ────────────────────────────────────────────────────────────

#[test]
fn next_id_empty_list_returns_one() {
    assert_eq!(next_id(&[]), Ok(1));
}

#[test]
fn next_id_non_empty_returns_max_plus_one() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "A"]), &store).unwrap();
    run_with_store(&args(&["add", "B"]), &store).unwrap();
    run_with_store(&args(&["add", "C"]), &store).unwrap();
    let todos = load_todos(&store).unwrap();
    // ids are 1, 2, 3 — next should be 4
    assert_eq!(next_id(&todos), Ok(4));
}

#[test]
fn next_id_overflow_returns_error() {
    use ona_rust::todo::Todo;
    let todos = vec![Todo {
        id: u32::MAX,
        text: "overflow".to_string(),
        done: false,
        category: None,
    }];
    assert!(next_id(&todos).is_err());
}

// ── cli commands via run_with_store ──────────────────────────────────────────

#[test]
fn add_and_list_todo() {
    let _dir = TempDir::new().unwrap();
    let store = Store::from_dir(_dir.path());
    run_with_store(&args(&["add", "Buy milk"]), &store).unwrap();
    let todos = load_todos(&store).unwrap();
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].text, "Buy milk");
    assert!(!todos[0].done);
    assert_eq!(todos[0].id, 1);
}

#[test]
fn add_multiple_todos_increments_id() {
    let _dir = TempDir::new().unwrap();
    let store = Store::from_dir(_dir.path());
    run_with_store(&args(&["add", "First"]), &store).unwrap();
    run_with_store(&args(&["add", "Second"]), &store).unwrap();
    let todos = load_todos(&store).unwrap();
    assert_eq!(todos[0].id, 1);
    assert_eq!(todos[1].id, 2);
}

#[test]
fn add_todo_with_category() {
    let _dir = TempDir::new().unwrap();
    let store = Store::from_dir(_dir.path());
    run_with_store(&args(&["add", "--cat", "work", "Write report"]), &store).unwrap();
    let todos = load_todos(&store).unwrap();
    assert!(matches!(todos[0].category, Some(Category::Work)));
}

#[test]
fn add_empty_text_returns_error() {
    let (store, _dir) = temp_store();
    assert!(run_with_store(&args(&["add", ""]), &store).is_err());
    assert!(run_with_store(&args(&["add", "   "]), &store).is_err());
    // No todos should have been created
    let todos = load_todos(&store).unwrap();
    assert!(todos.is_empty());
}

#[test]
fn add_todo_with_unknown_category_fails() {
    let _dir = TempDir::new().unwrap();
    let store = Store::from_dir(_dir.path());
    let result = run_with_store(&args(&["add", "--cat", "nonexistent", "Task"]), &store);
    assert!(result.is_err());
}

#[test]
fn done_marks_todo_complete() {
    let _dir = TempDir::new().unwrap();
    let store = Store::from_dir(_dir.path());
    run_with_store(&args(&["add", "Task"]), &store).unwrap();
    run_with_store(&args(&["done", "1"]), &store).unwrap();
    let todos = load_todos(&store).unwrap();
    assert!(todos[0].done);
}

#[test]
fn done_nonexistent_id_returns_error() {
    let _dir = TempDir::new().unwrap();
    let store = Store::from_dir(_dir.path());
    let result = run_with_store(&args(&["done", "99"]), &store);
    assert!(result.is_err());
}

#[test]
fn done_already_done_returns_error() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "Task"]), &store).unwrap();
    run_with_store(&args(&["done", "1"]), &store).unwrap();
    let result = run_with_store(&args(&["done", "1"]), &store);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already done"));
}

#[test]
fn remove_deletes_todo() {
    let _dir = TempDir::new().unwrap();
    let store = Store::from_dir(_dir.path());
    run_with_store(&args(&["add", "Task"]), &store).unwrap();
    run_with_store(&args(&["remove", "1"]), &store).unwrap();
    let todos = load_todos(&store).unwrap();
    assert!(todos.is_empty());
}

#[test]
fn remove_nonexistent_id_returns_error() {
    let _dir = TempDir::new().unwrap();
    let store = Store::from_dir(_dir.path());
    let result = run_with_store(&args(&["remove", "42"]), &store);
    assert!(result.is_err());
}

#[test]
fn list_empty_store_succeeds() {
    let _dir = TempDir::new().unwrap();
    let store = Store::from_dir(_dir.path());
    assert!(run_with_store(&args(&["list"]), &store).is_ok());
}

#[test]
fn category_add_and_use() {
    let _dir = TempDir::new().unwrap();
    let store = Store::from_dir(_dir.path());
    run_with_store(&args(&["category", "add", "hobby"]), &store).unwrap();
    let cats = load_custom_categories(&store).unwrap();
    assert_eq!(cats, vec!["hobby"]);

    // Can now use the custom category when adding a todo
    run_with_store(&args(&["add", "--cat", "hobby", "Paint"]), &store).unwrap();
    let todos = load_todos(&store).unwrap();
    assert!(matches!(&todos[0].category, Some(Category::Custom(s)) if s == "hobby"));
}

#[test]
fn category_add_duplicate_returns_error() {
    let _dir = TempDir::new().unwrap();
    let store = Store::from_dir(_dir.path());
    run_with_store(&args(&["category", "add", "hobby"]), &store).unwrap();
    let result = run_with_store(&args(&["category", "add", "hobby"]), &store);
    assert!(result.is_err());
}

#[test]
fn category_add_builtin_name_returns_error() {
    let _dir = TempDir::new().unwrap();
    let store = Store::from_dir(_dir.path());
    let result = run_with_store(&args(&["category", "add", "work"]), &store);
    assert!(result.is_err());
}

#[test]
fn category_remove_deletes_category() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["category", "add", "hobby"]), &store).unwrap();
    run_with_store(&args(&["category", "remove", "hobby"]), &store).unwrap();
    let cats = load_custom_categories(&store).unwrap();
    assert!(cats.is_empty());
}

#[test]
fn category_remove_nonexistent_returns_error() {
    let (store, _dir) = temp_store();
    let result = run_with_store(&args(&["category", "remove", "hobby"]), &store);
    assert!(result.is_err());
}

#[test]
fn category_remove_builtin_returns_error() {
    let (store, _dir) = temp_store();
    let result = run_with_store(&args(&["category", "remove", "work"]), &store);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("built-in"));
}

#[test]
fn category_edit_renames_category() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["category", "add", "hobby"]), &store).unwrap();
    run_with_store(&args(&["category", "edit", "hobby", "crafts"]), &store).unwrap();
    let cats = load_custom_categories(&store).unwrap();
    assert!(cats.contains(&"crafts".to_string()));
    assert!(!cats.contains(&"hobby".to_string()));
}

#[test]
fn category_edit_nonexistent_returns_error() {
    let (store, _dir) = temp_store();
    let result = run_with_store(&args(&["category", "edit", "hobby", "crafts"]), &store);
    assert!(result.is_err());
}

#[test]
fn category_edit_builtin_returns_error() {
    let (store, _dir) = temp_store();
    let result = run_with_store(&args(&["category", "edit", "work", "jobs"]), &store);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("built-in"));
}

#[test]
fn category_edit_to_existing_name_returns_error() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["category", "add", "hobby"]), &store).unwrap();
    run_with_store(&args(&["category", "add", "crafts"]), &store).unwrap();
    let result = run_with_store(&args(&["category", "edit", "hobby", "crafts"]), &store);
    assert!(result.is_err());
}

#[test]
fn category_edit_to_builtin_name_returns_error() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["category", "add", "hobby"]), &store).unwrap();
    let result = run_with_store(&args(&["category", "edit", "hobby", "work"]), &store);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("built-in"));
}

#[test]
fn category_remove_clears_category_on_todos() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["category", "add", "hobby"]), &store).unwrap();
    run_with_store(&args(&["add", "--cat", "hobby", "Paint"]), &store).unwrap();
    run_with_store(&args(&["category", "remove", "hobby"]), &store).unwrap();
    let todos = load_todos(&store).unwrap();
    assert!(todos[0].category.is_none());
}

#[test]
fn category_edit_updates_category_on_todos() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["category", "add", "hobby"]), &store).unwrap();
    run_with_store(&args(&["add", "--cat", "hobby", "Paint"]), &store).unwrap();
    run_with_store(&args(&["category", "edit", "hobby", "crafts"]), &store).unwrap();
    let todos = load_todos(&store).unwrap();
    assert!(matches!(&todos[0].category, Some(Category::Custom(s)) if s == "crafts"));
}

#[test]
fn invalid_id_parse_returns_error() {
    let _dir = TempDir::new().unwrap();
    let store = Store::from_dir(_dir.path());
    assert!(run_with_store(&args(&["done", "abc"]), &store).is_err());
    assert!(run_with_store(&args(&["remove", "abc"]), &store).is_err());
    assert!(run_with_store(&args(&["edit", "abc", "text"]), &store).is_err());
}

// ── edit command ─────────────────────────────────────────────────────────────

#[test]
fn edit_updates_todo_text() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "Old text"]), &store).unwrap();
    run_with_store(&args(&["edit", "1", "New text"]), &store).unwrap();
    let todos = load_todos(&store).unwrap();
    assert_eq!(todos[0].text, "New text");
}

#[test]
fn edit_preserves_done_and_category() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "--cat", "work", "Task"]), &store).unwrap();
    run_with_store(&args(&["done", "1"]), &store).unwrap();
    run_with_store(&args(&["edit", "1", "Updated task"]), &store).unwrap();
    let todos = load_todos(&store).unwrap();
    assert!(todos[0].done);
    assert!(matches!(todos[0].category, Some(Category::Work)));
    assert_eq!(todos[0].text, "Updated task");
}

#[test]
fn edit_multiword_text() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "Short"]), &store).unwrap();
    run_with_store(&args(&["edit", "1", "Buy", "milk", "and", "eggs"]), &store).unwrap();
    let todos = load_todos(&store).unwrap();
    assert_eq!(todos[0].text, "Buy milk and eggs");
}

#[test]
fn edit_nonexistent_id_returns_error() {
    let (store, _dir) = temp_store();
    let result = run_with_store(&args(&["edit", "99", "text"]), &store);
    assert!(result.is_err());
}

#[test]
fn edit_missing_args_returns_error() {
    let (store, _dir) = temp_store();
    assert!(run_with_store(&args(&["edit", "1"]), &store).is_err());
    assert!(run_with_store(&args(&["edit"]), &store).is_err());
}

#[test]
fn edit_empty_text_returns_error() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "Task"]), &store).unwrap();
    assert!(run_with_store(&args(&["edit", "1", ""]), &store).is_err());
    assert!(run_with_store(&args(&["edit", "1", "   "]), &store).is_err());
    // Original text must be unchanged
    let todos = load_todos(&store).unwrap();
    assert_eq!(todos[0].text, "Task");
}
