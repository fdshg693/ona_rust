use crate::api::extractor::AuthUser;
use crate::api::state::AppState;
use crate::category::parse_category;
use crate::cli::PAGE_SIZE;
use crate::todo::{
    load_custom_categories, load_todos, next_id, remove_category_atomic, rename_category_atomic,
    save_custom_categories, save_todos, Todo,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

// ── Shared helpers ────────────────────────────────────────────────────────────

fn internal(e: String) -> (StatusCode, String) {
    // Log the real error server-side; return a generic message to the client.
    eprintln!("internal error: {e}");
    (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error.".to_string())
}

fn not_found(msg: String) -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, msg)
}

fn bad_request(msg: String) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, msg)
}

fn conflict(msg: String) -> (StatusCode, String) {
    (StatusCode::CONFLICT, msg)
}

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AddTodoRequest {
    pub text: String,
    pub category: Option<String>,
}

#[derive(Deserialize)]
pub struct EditTodoRequest {
    pub text: String,
}

#[derive(Deserialize)]
pub struct PageQuery {
    pub page: Option<usize>,
}

#[derive(Serialize)]
pub struct TodosPage {
    pub todos: Vec<Todo>,
    pub page: usize,
    pub total_pages: usize,
}

#[derive(Deserialize)]
pub struct AddCategoryRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct EditCategoryRequest {
    pub new_name: String,
}

#[derive(Serialize)]
pub struct CategoriesResponse {
    pub builtin: Vec<&'static str>,
    pub custom: Vec<String>,
}

// ── Todo handlers ─────────────────────────────────────────────────────────────

/// `GET /todos?page=<n>` — list todos, 10 per page (default page 1).
pub async fn list_todos(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Query(q): Query<PageQuery>,
) -> impl IntoResponse {
    let page = q.page.unwrap_or(1);
    if page == 0 {
        return bad_request("Page number must be 1 or greater.".to_string()).into_response();
    }

    let todos = match load_todos(&state.store) {
        Ok(t) => t,
        Err(e) => return internal(e).into_response(),
    };

    let total_pages = todos.len().div_ceil(PAGE_SIZE).max(1);
    if page > total_pages {
        return bad_request(format!(
            "Page {page} out of range (total pages: {total_pages})."
        ))
        .into_response();
    }

    let start = (page - 1) * PAGE_SIZE;
    let end = (start + PAGE_SIZE).min(todos.len());
    Json(TodosPage {
        todos: todos[start..end].to_vec(),
        page,
        total_pages,
    })
    .into_response()
}

/// `POST /todos` — add a new todo.
pub async fn add_todo(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Json(body): Json<AddTodoRequest>,
) -> impl IntoResponse {
    if body.text.trim().is_empty() {
        return bad_request("Todo text cannot be empty.".to_string()).into_response();
    }

    let store = &state.store;
    let custom_cats = match load_custom_categories(store) {
        Ok(c) => c,
        Err(e) => return internal(e).into_response(),
    };

    let category = match body.category {
        Some(ref name) => match parse_category(name, &custom_cats) {
            Ok(c) => Some(c),
            Err(e) => return bad_request(e).into_response(),
        },
        None => None,
    };

    let mut todos = match load_todos(store) {
        Ok(t) => t,
        Err(e) => return internal(e).into_response(),
    };

    let id = match next_id(&todos) {
        Ok(id) => id,
        Err(e) => return internal(e).into_response(),
    };

    let todo = Todo {
        id,
        text: body.text.clone(),
        done: false,
        category,
    };
    todos.push(todo.clone());

    match save_todos(store, &todos) {
        Ok(()) => (StatusCode::CREATED, Json(todo)).into_response(),
        Err(e) => internal(e).into_response(),
    }
}

/// `PATCH /todos/:id/done` — mark a todo as done.
pub async fn mark_done(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Path(id): Path<u32>,
) -> impl IntoResponse {
    let store = &state.store;
    let mut todos = match load_todos(store) {
        Ok(t) => t,
        Err(e) => return internal(e).into_response(),
    };

    match todos.iter_mut().find(|t| t.id == id) {
        None => not_found(format!("Todo #{id} not found.")).into_response(),
        Some(t) if t.done => {
            bad_request(format!("Todo #{id} is already done.")).into_response()
        }
        Some(t) => {
            t.done = true;
            let updated = t.clone();
            match save_todos(store, &todos) {
                Ok(()) => Json(updated).into_response(),
                Err(e) => internal(e).into_response(),
            }
        }
    }
}

/// `PUT /todos/:id` — update the text of a todo.
pub async fn edit_todo(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Path(id): Path<u32>,
    Json(body): Json<EditTodoRequest>,
) -> impl IntoResponse {
    if body.text.trim().is_empty() {
        return bad_request("Todo text cannot be empty.".to_string()).into_response();
    }

    let store = &state.store;
    let mut todos = match load_todos(store) {
        Ok(t) => t,
        Err(e) => return internal(e).into_response(),
    };

    match todos.iter_mut().find(|t| t.id == id) {
        None => not_found(format!("Todo #{id} not found.")).into_response(),
        Some(t) => {
            t.text = body.text.clone();
            let updated = t.clone();
            match save_todos(store, &todos) {
                Ok(()) => Json(updated).into_response(),
                Err(e) => internal(e).into_response(),
            }
        }
    }
}

/// `DELETE /todos/:id` — remove a todo.
pub async fn remove_todo(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Path(id): Path<u32>,
) -> impl IntoResponse {
    let store = &state.store;
    let mut todos = match load_todos(store) {
        Ok(t) => t,
        Err(e) => return internal(e).into_response(),
    };

    let len = todos.len();
    todos.retain(|t| t.id != id);
    if todos.len() == len {
        return not_found(format!("Todo #{id} not found.")).into_response();
    }

    match save_todos(store, &todos) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => internal(e).into_response(),
    }
}

// ── Category handlers ─────────────────────────────────────────────────────────

use crate::category::BUILTIN_CATEGORIES;

/// `GET /categories` — list all categories.
pub async fn list_categories(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
) -> impl IntoResponse {
    match load_custom_categories(&state.store) {
        Ok(custom) => Json(CategoriesResponse {
            builtin: BUILTIN_CATEGORIES.to_vec(),
            custom,
        })
        .into_response(),
        Err(e) => internal(e).into_response(),
    }
}

/// `POST /categories` — add a custom category.
pub async fn add_category(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Json(body): Json<AddCategoryRequest>,
) -> impl IntoResponse {
    let lower = body.name.to_lowercase();
    if BUILTIN_CATEGORIES.contains(&lower.as_str()) {
        return bad_request(format!("'{}' is a built-in category.", body.name)).into_response();
    }

    let store = &state.store;
    let mut cats = match load_custom_categories(store) {
        Ok(c) => c,
        Err(e) => return internal(e).into_response(),
    };

    if cats.iter().any(|c| c.to_lowercase() == lower) {
        return conflict(format!("Category '{}' already exists.", body.name)).into_response();
    }

    cats.push(body.name.clone());
    match save_custom_categories(store, &cats) {
        Ok(()) => (StatusCode::CREATED, Json(serde_json::json!({ "name": body.name }))).into_response(),
        Err(e) => internal(e).into_response(),
    }
}

/// `PUT /categories/:name` — rename a custom category.
pub async fn edit_category(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Path(old_name): Path<String>,
    Json(body): Json<EditCategoryRequest>,
) -> impl IntoResponse {
    let old_lower = old_name.to_lowercase();
    let new_lower = body.new_name.to_lowercase();

    if BUILTIN_CATEGORIES.contains(&old_lower.as_str()) {
        return bad_request(format!("'{}' is a built-in category and cannot be renamed.", old_name))
            .into_response();
    }
    if body.new_name.trim().is_empty() {
        return bad_request("Category name cannot be empty.".to_string()).into_response();
    }
    if BUILTIN_CATEGORIES.contains(&new_lower.as_str()) {
        return bad_request(format!("'{}' is a built-in category name.", body.new_name))
            .into_response();
    }

    let store = &state.store;
    let mut cats = match load_custom_categories(store) {
        Ok(c) => c,
        Err(e) => return internal(e).into_response(),
    };

    let pos = match cats.iter().position(|c| c.to_lowercase() == old_lower) {
        Some(p) => p,
        None => return not_found(format!("Category '{}' not found.", old_name)).into_response(),
    };

    if cats.iter().any(|c| c.to_lowercase() == new_lower) {
        return conflict(format!("Category '{}' already exists.", body.new_name)).into_response();
    }

    cats[pos] = body.new_name.clone();
    if let Err(e) = rename_category_atomic(store, &cats, &old_name, &body.new_name) {
        return internal(e).into_response();
    }

    Json(serde_json::json!({ "name": body.new_name })).into_response()
}

/// `DELETE /categories/:name` — remove a custom category.
pub async fn remove_category(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let lower = name.to_lowercase();
    if BUILTIN_CATEGORIES.contains(&lower.as_str()) {
        return bad_request(format!(
            "'{}' is a built-in category and cannot be removed.",
            name
        ))
        .into_response();
    }

    let store = &state.store;
    let mut cats = match load_custom_categories(store) {
        Ok(c) => c,
        Err(e) => return internal(e).into_response(),
    };

    let len = cats.len();
    cats.retain(|c| c.to_lowercase() != lower);
    if cats.len() == len {
        return not_found(format!("Category '{}' not found.", name)).into_response();
    }

    if let Err(e) = remove_category_atomic(store, &cats, &name) {
        return internal(e).into_response();
    }

    StatusCode::NO_CONTENT.into_response()
}
