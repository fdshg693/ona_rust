use ona_rust::auth::{cmd_login, cmd_logout, cmd_register, read_session, set_session_path_for_test};
use ona_rust::category::{parse_category, Category};
use ona_rust::cli::{cmd_list, run_with_store, PAGE_SIZE};
use ona_rust::storage::Store;
use ona_rust::todo::{load_custom_categories, load_todos, next_id, save_todos, Todo};
use tempfile::TempDir;

fn args(parts: &[&str]) -> Vec<String> {
    // Prepend a fake argv[0] to match real CLI layout
    std::iter::once("todo")
        .chain(parts.iter().copied())
        .map(String::from)
        .collect()
}

/// Creates a temp store and a pre-authenticated session for the current thread.
/// All tests that call `run_with_store` for todo commands must use this helper
/// because `run_with_store` calls `require_auth()` before dispatching.
fn temp_store() -> (Store, TempDir) {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path());
    // Use a thread-local session path so parallel tests don't interfere.
    set_session_path_for_test(dir.path().join("session"));
    // Register a test user so require_auth() succeeds.
    cmd_register(&store, "testuser", "testpass").unwrap();
    (store, dir)
}

fn load_todos_for_user(store: &Store, username: &str) -> Result<Vec<Todo>, String> {
    let conn = store.open()?;
    load_todos(&conn, username)
}

/// Helper: register a user and confirm the session is written.
fn register_user(store: &Store, username: &str, password: &str) {
    cmd_register(store, username, password).unwrap();
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
    let (store, _dir) = temp_store();
    let conn = store.open().unwrap();
    assert_eq!(next_id(&conn), Ok(1));
}

#[test]
fn next_id_non_empty_returns_max_plus_one() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "A"]), &store).unwrap();
    run_with_store(&args(&["add", "B"]), &store).unwrap();
    run_with_store(&args(&["add", "C"]), &store).unwrap();
    let conn = store.open().unwrap();
    // ids are 1, 2, 3 — next should be 4
    assert_eq!(next_id(&conn), Ok(4));
}

#[test]
fn next_id_overflow_returns_error() {
    let (store, _dir) = temp_store();
    let conn = store.open().unwrap();
    let overflow = vec![Todo {
        id: u32::MAX,
        text: "overflow".to_string(),
        done: false,
        category: None,
        owner: Some("testuser".to_string()),
    }];
    save_todos(&conn, &overflow).unwrap();
    assert!(next_id(&conn).is_err());
}

// ── cli commands via run_with_store ──────────────────────────────────────────

#[test]
fn add_and_list_todo() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "Buy milk"]), &store).unwrap();
    let todos = load_todos_for_user(&store, "testuser").unwrap();
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].text, "Buy milk");
    assert!(!todos[0].done);
    assert_eq!(todos[0].id, 1);
}

#[test]
fn add_multiple_todos_increments_id() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "First"]), &store).unwrap();
    run_with_store(&args(&["add", "Second"]), &store).unwrap();
    let todos = load_todos_for_user(&store, "testuser").unwrap();
    assert_eq!(todos[0].id, 1);
    assert_eq!(todos[1].id, 2);
}

#[test]
fn add_todo_with_category() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "--cat", "work", "Write report"]), &store).unwrap();
    let todos = load_todos_for_user(&store, "testuser").unwrap();
    assert!(matches!(todos[0].category, Some(Category::Work)));
}

#[test]
fn add_empty_text_returns_error() {
    let (store, _dir) = temp_store();
    assert!(run_with_store(&args(&["add", ""]), &store).is_err());
    assert!(run_with_store(&args(&["add", "   "]), &store).is_err());
    // No todos should have been created
    let todos = load_todos_for_user(&store, "testuser").unwrap();
    assert!(todos.is_empty());
}

#[test]
fn add_todo_with_unknown_category_fails() {
    let (store, _dir) = temp_store();
    let result = run_with_store(&args(&["add", "--cat", "nonexistent", "Task"]), &store);
    assert!(result.is_err());
}

#[test]
fn done_marks_todo_complete() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "Task"]), &store).unwrap();
    run_with_store(&args(&["done", "1"]), &store).unwrap();
    let todos = load_todos_for_user(&store, "testuser").unwrap();
    assert!(todos[0].done);
}

#[test]
fn done_nonexistent_id_returns_error() {
    let (store, _dir) = temp_store();
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
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "Task"]), &store).unwrap();
    run_with_store(&args(&["remove", "1"]), &store).unwrap();
    let todos = load_todos_for_user(&store, "testuser").unwrap();
    assert!(todos.is_empty());
}

#[test]
fn remove_nonexistent_id_returns_error() {
    let (store, _dir) = temp_store();
    let result = run_with_store(&args(&["remove", "42"]), &store);
    assert!(result.is_err());
}

#[test]
fn list_empty_store_succeeds() {
    let (store, _dir) = temp_store();
    assert!(run_with_store(&args(&["list"]), &store).is_ok());
}

#[test]
fn category_add_and_use() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["category", "add", "hobby"]), &store).unwrap();
    let cats = load_custom_categories(&store).unwrap();
    assert_eq!(cats, vec!["hobby"]);

    // Can now use the custom category when adding a todo
    run_with_store(&args(&["add", "--cat", "hobby", "Paint"]), &store).unwrap();
    let todos = load_todos_for_user(&store, "testuser").unwrap();
    assert!(matches!(&todos[0].category, Some(Category::Custom(s)) if s == "hobby"));
}

#[test]
fn category_add_duplicate_returns_error() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["category", "add", "hobby"]), &store).unwrap();
    let result = run_with_store(&args(&["category", "add", "hobby"]), &store);
    assert!(result.is_err());
}

#[test]
fn category_add_builtin_name_returns_error() {
    let (store, _dir) = temp_store();
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
    let todos = load_todos_for_user(&store, "testuser").unwrap();
    assert!(todos[0].category.is_none());
}

#[test]
fn category_edit_updates_category_on_todos() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["category", "add", "hobby"]), &store).unwrap();
    run_with_store(&args(&["add", "--cat", "hobby", "Paint"]), &store).unwrap();
    run_with_store(&args(&["category", "edit", "hobby", "crafts"]), &store).unwrap();
    let todos = load_todos_for_user(&store, "testuser").unwrap();
    assert!(matches!(&todos[0].category, Some(Category::Custom(s)) if s == "crafts"));
}

#[test]
fn invalid_id_parse_returns_error() {
    let (store, _dir) = temp_store();
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
    let todos = load_todos_for_user(&store, "testuser").unwrap();
    assert_eq!(todos[0].text, "New text");
}

#[test]
fn edit_preserves_done_and_category() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "--cat", "work", "Task"]), &store).unwrap();
    run_with_store(&args(&["done", "1"]), &store).unwrap();
    run_with_store(&args(&["edit", "1", "Updated task"]), &store).unwrap();
    let todos = load_todos_for_user(&store, "testuser").unwrap();
    assert!(todos[0].done);
    assert!(matches!(todos[0].category, Some(Category::Work)));
    assert_eq!(todos[0].text, "Updated task");
}

#[test]
fn edit_multiword_text() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "Short"]), &store).unwrap();
    run_with_store(&args(&["edit", "1", "Buy", "milk", "and", "eggs"]), &store).unwrap();
    let todos = load_todos_for_user(&store, "testuser").unwrap();
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
    let todos = load_todos_for_user(&store, "testuser").unwrap();
    assert_eq!(todos[0].text, "Task");
}

// ── list pagination ───────────────────────────────────────────────────────────

#[test]
fn list_page_one_of_one_succeeds() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "Task"]), &store).unwrap();
    assert!(cmd_list(&store, "testuser", 1).is_ok());
}

#[test]
fn list_page_zero_returns_error() {
    let (store, _dir) = temp_store();
    assert!(cmd_list(&store, "testuser", 0).is_err());
}

#[test]
fn list_page_out_of_range_returns_error() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "Task"]), &store).unwrap();
    // Only 1 todo → only 1 page
    assert!(cmd_list(&store, "testuser", 2).is_err());
}

#[test]
fn list_page_flag_via_run_with_store() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "Task"]), &store).unwrap();
    assert!(run_with_store(&args(&["list", "--page", "1"]), &store).is_ok());
}

#[test]
fn list_invalid_page_flag_returns_error() {
    let (store, _dir) = temp_store();
    assert!(run_with_store(&args(&["list", "--page", "abc"]), &store).is_err());
}

#[test]
fn list_page_zero_via_flag_returns_error() {
    let (store, _dir) = temp_store();
    assert!(run_with_store(&args(&["list", "--page", "0"]), &store).is_err());
}

#[test]
fn list_pagination_splits_correctly() {
    let (store, _dir) = temp_store();
    // Add PAGE_SIZE + 1 todos to force two pages
    for i in 1..=(PAGE_SIZE + 1) {
        run_with_store(&args(&["add", &format!("Task {i}")]), &store).unwrap();
    }
    assert!(cmd_list(&store, "testuser", 1).is_ok());
    assert!(cmd_list(&store, "testuser", 2).is_ok());
    // Page 3 does not exist
    assert!(cmd_list(&store, "testuser", 3).is_err());
}

#[test]
fn list_empty_store_page_one_succeeds() {
    let (store, _dir) = temp_store();
    // Empty list: no todos, page 1 is still valid
    assert!(cmd_list(&store, "testuser", 1).is_ok());
}

// ── auth commands ─────────────────────────────────────────────────────────────

#[test]
fn register_creates_user_and_writes_session() {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path());
    let session = dir.path().join("session");
    set_session_path_for_test(session);

    register_user(&store, "alice", "secret");

    let username = read_session().unwrap();
    assert_eq!(username, "alice");
}

#[test]
fn register_duplicate_username_returns_error() {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path());
    set_session_path_for_test(dir.path().join("session"));

    register_user(&store, "alice", "secret");
    let result = cmd_register(&store, "alice", "other");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already taken"));
}

#[test]
fn register_duplicate_username_case_insensitive() {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path());
    set_session_path_for_test(dir.path().join("session"));

    register_user(&store, "Alice", "secret");
    let result = cmd_register(&store, "alice", "other");
    assert!(result.is_err());
}

#[test]
fn register_empty_username_returns_error() {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path());
    set_session_path_for_test(dir.path().join("session"));

    assert!(cmd_register(&store, "", "secret").is_err());
    assert!(cmd_register(&store, "   ", "secret").is_err());
}

#[test]
fn register_empty_password_returns_error() {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path());
    set_session_path_for_test(dir.path().join("session"));

    assert!(cmd_register(&store, "alice", "").is_err());
}

#[test]
fn login_valid_credentials_writes_session() {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path());
    set_session_path_for_test(dir.path().join("session"));

    register_user(&store, "bob", "pass123");
    // Log out first so we can test login independently.
    cmd_logout().unwrap();

    cmd_login(&store, "bob", "pass123").unwrap();
    assert_eq!(read_session().unwrap(), "bob");
}

#[test]
fn login_wrong_password_returns_error() {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path());
    set_session_path_for_test(dir.path().join("session"));

    register_user(&store, "bob", "correct");
    cmd_logout().unwrap();

    let result = cmd_login(&store, "bob", "wrong");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid username or password"));
}

#[test]
fn login_unknown_user_returns_error() {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path());
    set_session_path_for_test(dir.path().join("session"));

    let result = cmd_login(&store, "nobody", "pass");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid username or password"));
}

#[test]
fn login_case_insensitive_username() {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path());
    set_session_path_for_test(dir.path().join("session"));

    register_user(&store, "Carol", "pass");
    cmd_logout().unwrap();

    cmd_login(&store, "carol", "pass").unwrap();
    // Session stores the canonical casing from registration.
    assert_eq!(read_session().unwrap(), "Carol");
}

#[test]
fn logout_clears_session() {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path());
    set_session_path_for_test(dir.path().join("session"));

    register_user(&store, "dave", "pass");
    cmd_logout().unwrap();

    assert!(read_session().is_err());
}

#[test]
fn todo_commands_blocked_without_session() {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path());
    // Point to a session file that does not exist.
    set_session_path_for_test(dir.path().join("no_session"));

    let result = run_with_store(&args(&["add", "Task"]), &store);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Not logged in"));
}

#[test]
fn todo_commands_work_after_login() {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path());
    set_session_path_for_test(dir.path().join("session"));

    register_user(&store, "eve", "pass");
    run_with_store(&args(&["add", "My task"]), &store).unwrap();

    let todos = load_todos_for_user(&store, "eve").unwrap();
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].text, "My task");
}

#[test]
fn register_and_login_commands_via_run_with_store() {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path());
    set_session_path_for_test(dir.path().join("session"));

    run_with_store(&args(&["register", "frank", "pass"]), &store).unwrap();
    run_with_store(&args(&["logout"]), &store).unwrap();
    run_with_store(&args(&["login", "frank", "pass"]), &store).unwrap();

    assert_eq!(read_session().unwrap(), "frank");
}

// ── multi-user isolation ─────────────────────────────────────────────────────

#[test]
fn two_users_see_own_todos_not_each_others() {
    let (store, _dir) = temp_store();
    cmd_register(&store, "otheruser", "otherpass").unwrap();
    cmd_login(&store, "testuser", "testpass").unwrap();
    run_with_store(&args(&["add", "testuser todo"]), &store).unwrap();
    cmd_login(&store, "otheruser", "otherpass").unwrap();
    run_with_store(&args(&["add", "otheruser todo"]), &store).unwrap();

    let testuser_todos = load_todos_for_user(&store, "testuser").unwrap();
    assert_eq!(testuser_todos.len(), 1);
    assert_eq!(testuser_todos[0].text, "testuser todo");

    let otheruser_todos = load_todos_for_user(&store, "otheruser").unwrap();
    assert_eq!(otheruser_todos.len(), 1);
    assert_eq!(otheruser_todos[0].text, "otheruser todo");
}

#[test]
fn shared_todo_visible_to_all_users() {
    let (store, _dir) = temp_store();
    cmd_register(&store, "otheruser", "otherpass").unwrap();
    let conn = store.open().unwrap();
    conn
        .execute(
            "INSERT INTO todos (id, text, done, owner) VALUES (1, 'shared task', 0, NULL)",
            [],
        )
        .unwrap();

    let testuser_todos = load_todos_for_user(&store, "testuser").unwrap();
    assert!(
        testuser_todos.iter().any(|t| t.text == "shared task"),
        "testuser should see shared todo"
    );

    let otheruser_todos = load_todos_for_user(&store, "otheruser").unwrap();
    assert!(
        otheruser_todos.iter().any(|t| t.text == "shared task"),
        "otheruser should see shared todo"
    );
}

#[test]
fn done_command_scoped_to_user() {
    let (store, _dir) = temp_store();
    run_with_store(&args(&["add", "testuser task"]), &store).unwrap();
    cmd_register(&store, "otheruser", "otherpass").unwrap();
    run_with_store(&args(&["add", "otheruser task"]), &store).unwrap();
    cmd_login(&store, "testuser", "testpass").unwrap();
    run_with_store(&args(&["done", "1"]), &store).unwrap();

    let testuser_todos = load_todos_for_user(&store, "testuser").unwrap();
    let t1 = testuser_todos.iter().find(|t| t.id == 1).expect("testuser todo id 1");
    assert!(t1.done);

    let otheruser_todos = load_todos_for_user(&store, "otheruser").unwrap();
    let t2 = otheruser_todos
        .iter()
        .find(|t| t.id == 2)
        .expect("otheruser todo id 2");
    assert!(!t2.done);
}
