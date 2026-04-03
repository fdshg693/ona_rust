use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use ona_rust::api::{build_router, state::AppState};
use ona_rust::storage::Store;
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;

// ── Test helpers ──────────────────────────────────────────────────────────────

fn make_app(dir: &TempDir) -> axum::Router {
    let store = Store::from_dir(dir.path());
    build_router(AppState { store })
}

async fn body_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn body_text(body: Body) -> String {
    let bytes = body.collect().await.unwrap().to_bytes();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// POST JSON to `path`, return (status, body).
async fn post_json(app: axum::Router, path: &str, payload: Value) -> (StatusCode, Body) {
    let req = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    (status, resp.into_body())
}

/// POST JSON with Bearer token.
async fn post_json_auth(
    app: axum::Router,
    path: &str,
    token: &str,
    payload: Value,
) -> (StatusCode, Body) {
    let req = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(payload.to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    (status, resp.into_body())
}

/// GET with Bearer token.
async fn get_auth(app: axum::Router, path: &str, token: &str) -> (StatusCode, Body) {
    let req = Request::builder()
        .method(Method::GET)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    (status, resp.into_body())
}

/// Register a user and return the token.
async fn register_and_token(app: axum::Router, username: &str, password: &str) -> String {
    let (status, body) = post_json(
        app,
        "/auth/register",
        json!({ "username": username, "password": password }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "register failed");
    body_json(body).await["token"]
        .as_str()
        .unwrap()
        .to_owned()
}

// ── Auth tests ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn register_returns_token() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let (status, body) = post_json(
        app,
        "/auth/register",
        json!({ "username": "alice", "password": "secret" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let json = body_json(body).await;
    assert!(json["token"].is_string());
}

#[tokio::test]
async fn register_duplicate_username_returns_conflict() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    post_json(
        app.clone(),
        "/auth/register",
        json!({ "username": "alice", "password": "secret" }),
    )
    .await;
    let (status, _) = post_json(
        app,
        "/auth/register",
        json!({ "username": "alice", "password": "other" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn login_valid_credentials_returns_token() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    post_json(
        app.clone(),
        "/auth/register",
        json!({ "username": "bob", "password": "pass" }),
    )
    .await;
    let (status, body) = post_json(
        app,
        "/auth/login",
        json!({ "username": "bob", "password": "pass" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body_json(body).await["token"].is_string());
}

#[tokio::test]
async fn login_wrong_password_returns_401() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    post_json(
        app.clone(),
        "/auth/register",
        json!({ "username": "carol", "password": "right" }),
    )
    .await;
    let (status, _) = post_json(
        app,
        "/auth/login",
        json!({ "username": "carol", "password": "wrong" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_revokes_token() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "dave", "pw").await;

    // Logout
    let req = Request::builder()
        .method(Method::POST)
        .uri("/auth/logout")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Token should now be invalid
    let (status, _) = get_auth(app, "/todos", &token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unauthenticated_request_returns_401() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/todos")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ── Todo tests ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn add_and_list_todo() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "eve", "pw").await;

    let (status, body) = post_json_auth(
        app.clone(),
        "/todos",
        &token,
        json!({ "text": "buy milk" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let created = body_json(body).await;
    assert_eq!(created["text"], "buy milk");
    assert_eq!(created["done"], false);
    let id = created["id"].as_u64().unwrap();

    let (status, body) = get_auth(app, "/todos", &token).await;
    assert_eq!(status, StatusCode::OK);
    let page = body_json(body).await;
    assert_eq!(page["todos"][0]["id"], id);
}

#[tokio::test]
async fn add_todo_empty_text_returns_400() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "frank", "pw").await;
    let (status, _) =
        post_json_auth(app, "/todos", &token, json!({ "text": "   " })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn mark_todo_done() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "grace", "pw").await;

    let (_, body) =
        post_json_auth(app.clone(), "/todos", &token, json!({ "text": "task" })).await;
    let id = body_json(body).await["id"].as_u64().unwrap();

    let req = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/todos/{id}/done"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp.into_body()).await;
    assert_eq!(json["done"], true);
}

#[tokio::test]
async fn mark_done_nonexistent_returns_404() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "grace2", "pw").await;

    let req = Request::builder()
        .method(Method::PATCH)
        .uri("/todos/999/done")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn mark_done_already_done_returns_400() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "grace3", "pw").await;

    let (_, body) =
        post_json_auth(app.clone(), "/todos", &token, json!({ "text": "task" })).await;
    let id = body_json(body).await["id"].as_u64().unwrap();

    // First mark — should succeed
    let req = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/todos/{id}/done"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req).await.unwrap();

    // Second mark — should be 400
    let req = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/todos/{id}/done"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn edit_todo_text() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "heidi", "pw").await;

    let (_, body) =
        post_json_auth(app.clone(), "/todos", &token, json!({ "text": "old" })).await;
    let id = body_json(body).await["id"].as_u64().unwrap();

    let req = Request::builder()
        .method(Method::PUT)
        .uri(format!("/todos/{id}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(json!({ "text": "new" }).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp.into_body()).await;
    assert_eq!(json["text"], "new");
}

#[tokio::test]
async fn remove_todo() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "ivan", "pw").await;

    let (_, body) =
        post_json_auth(app.clone(), "/todos", &token, json!({ "text": "delete me" })).await;
    let id = body_json(body).await["id"].as_u64().unwrap();

    let req = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/todos/{id}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Confirm it's gone
    let (_, body) = get_auth(app, "/todos", &token).await;
    let page = body_json(body).await;
    assert_eq!(page["todos"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn remove_nonexistent_todo_returns_404() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "judy", "pw").await;

    let req = Request::builder()
        .method(Method::DELETE)
        .uri("/todos/999")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn add_todo_with_category() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "kate", "pw").await;

    let (status, body) = post_json_auth(
        app,
        "/todos",
        &token,
        json!({ "text": "gym", "category": "health" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let json = body_json(body).await;
    assert_eq!(json["category"], "health");
}

#[tokio::test]
async fn add_todo_with_unknown_category_returns_400() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "leo", "pw").await;

    let (status, _) = post_json_auth(
        app,
        "/todos",
        &token,
        json!({ "text": "task", "category": "nonexistent" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── Category tests ────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_categories_returns_builtins() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "mia", "pw").await;

    let (status, body) = get_auth(app, "/categories", &token).await;
    assert_eq!(status, StatusCode::OK);
    let json = body_json(body).await;
    let builtins = json["builtin"].as_array().unwrap();
    assert!(builtins.iter().any(|v| v == "work"));
    assert!(builtins.iter().any(|v| v == "health"));
}

#[tokio::test]
async fn add_and_use_custom_category() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "noah", "pw").await;

    let (status, _) = post_json_auth(
        app.clone(),
        "/categories",
        &token,
        json!({ "name": "hobby" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) = post_json_auth(
        app,
        "/todos",
        &token,
        json!({ "text": "paint", "category": "hobby" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body_json(body).await["category"], "hobby");
}

#[tokio::test]
async fn add_builtin_category_returns_400() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "olivia", "pw").await;

    let (status, _) = post_json_auth(
        app,
        "/categories",
        &token,
        json!({ "name": "work" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn edit_category() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "peter", "pw").await;

    post_json_auth(
        app.clone(),
        "/categories",
        &token,
        json!({ "name": "oldcat" }),
    )
    .await;

    let req = Request::builder()
        .method(Method::PUT)
        .uri("/categories/oldcat")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(json!({ "new_name": "newcat" }).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp.into_body()).await;
    assert_eq!(json["name"], "newcat");
}

#[tokio::test]
async fn remove_category() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "quinn", "pw").await;

    post_json_auth(
        app.clone(),
        "/categories",
        &token,
        json!({ "name": "temp" }),
    )
    .await;

    let req = Request::builder()
        .method(Method::DELETE)
        .uri("/categories/temp")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Confirm it's gone from the list
    let (_, body) = get_auth(app, "/categories", &token).await;
    let json = body_json(body).await;
    let custom = json["custom"].as_array().unwrap();
    assert!(!custom.iter().any(|v| v == "temp"));
}

#[tokio::test]
async fn list_todos_pagination() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token = register_and_token(app.clone(), "rose", "pw").await;

    // Add 12 todos
    for i in 0..12 {
        post_json_auth(
            app.clone(),
            "/todos",
            &token,
            json!({ "text": format!("todo {i}") }),
        )
        .await;
    }

    let (status, body) = get_auth(app.clone(), "/todos?page=1", &token).await;
    assert_eq!(status, StatusCode::OK);
    let page1 = body_json(body).await;
    assert_eq!(page1["todos"].as_array().unwrap().len(), 10);
    assert_eq!(page1["total_pages"], 2);

    let (status, body) = get_auth(app, "/todos?page=2", &token).await;
    assert_eq!(status, StatusCode::OK);
    let page2 = body_json(body).await;
    assert_eq!(page2["todos"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn invalid_token_returns_401() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let (status, _) = get_auth(app, "/todos", "not-a-real-token").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── Multi-user isolation tests ────────────────────────────────────────────────

#[tokio::test]
async fn api_two_users_cannot_see_each_others_todos() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token_a = register_and_token(app.clone(), "alice", "pass1").await;
    let token_b = register_and_token(app.clone(), "bob", "pass2").await;

    let (status, _) = post_json_auth(
        app.clone(),
        "/todos",
        &token_a,
        json!({ "text": "alice task" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, _) = post_json_auth(
        app.clone(),
        "/todos",
        &token_b,
        json!({ "text": "bob task" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) = get_auth(app.clone(), "/todos", &token_a).await;
    assert_eq!(status, StatusCode::OK);
    let page = body_json(body).await;
    let todos_a = page["todos"].as_array().unwrap();
    assert_eq!(todos_a.len(), 1);
    assert_eq!(todos_a[0]["text"], "alice task");

    let (status, body) = get_auth(app, "/todos", &token_b).await;
    assert_eq!(status, StatusCode::OK);
    let page = body_json(body).await;
    let todos_b = page["todos"].as_array().unwrap();
    assert_eq!(todos_b.len(), 1);
    assert_eq!(todos_b[0]["text"], "bob task");
}

#[tokio::test]
async fn api_shared_todo_visible_to_all_users() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token_a = register_and_token(app.clone(), "alice", "pass1").await;
    let token_b = register_and_token(app.clone(), "bob", "pass2").await;

    Store::from_dir(dir.path())
        .open()
        .unwrap()
        .execute(
            "INSERT INTO todos (id, text, done, owner) VALUES (1, 'shared', 0, NULL)",
            [],
        )
        .unwrap();

    for token in [&token_a, &token_b] {
        let (status, body) = get_auth(app.clone(), "/todos", token).await;
        assert_eq!(status, StatusCode::OK);
        let page = body_json(body).await;
        let texts: Vec<&str> = page["todos"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["text"].as_str())
            .collect();
        assert!(
            texts.contains(&"shared"),
            "expected 'shared' in {:?}",
            texts
        );
    }
}

#[tokio::test]
async fn api_user_cannot_see_other_users_todo_by_id() {
    let dir = TempDir::new().unwrap();
    let app = make_app(&dir);
    let token_a = register_and_token(app.clone(), "alice", "pass1").await;
    let token_b = register_and_token(app.clone(), "bob", "pass2").await;

    let (status, body) = post_json_auth(
        app.clone(),
        "/todos",
        &token_a,
        json!({ "text": "alice only" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = body_json(body).await["id"].as_u64().unwrap();

    let (status, body) = get_auth(app.clone(), "/todos", &token_b).await;
    assert_eq!(status, StatusCode::OK);
    let page = body_json(body).await;
    assert_eq!(page["todos"].as_array().unwrap().len(), 0);

    let req = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/todos/{id}/done"))
        .header(header::AUTHORIZATION, format!("Bearer {token_b}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// Suppress unused warning for body_text helper (available for debugging)
#[allow(dead_code)]
fn _use_body_text() {
    let _ = body_text;
}
