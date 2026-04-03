pub mod auth_handlers;
pub mod extractor;
pub mod state;
pub mod todo_handlers;

use crate::api::{
    auth_handlers::{login, logout, register},
    state::AppState,
    todo_handlers::{
        add_category, add_todo, edit_category, edit_todo, list_categories, list_todos, mark_done,
        remove_category, remove_todo,
    },
};
use axum::{
    routing::{get, patch, post, put},
    Router,
};
use tower_http::cors::CorsLayer;

/// Build the Axum router with all routes and middleware attached.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Auth
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        // Todos
        .route("/todos", get(list_todos).post(add_todo))
        .route("/todos/:id", put(edit_todo).delete(remove_todo))
        .route("/todos/:id/done", patch(mark_done))
        // Categories
        .route("/categories", get(list_categories).post(add_category))
        .route("/categories/:name", put(edit_category).delete(remove_category))
        // Middleware
        .layer(CorsLayer::permissive())
        .with_state(state)
}
