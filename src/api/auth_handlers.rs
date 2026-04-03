use crate::api::extractor::AuthUser;
use crate::api::state::AppState;
use crate::auth::{create_token, db_create_user, db_verify_credentials, revoke_token};
use axum::{
    extract::State,
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub token: String,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `POST /auth/register` — create a new user and return a session token.
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> impl IntoResponse {
    if body.username.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "Username cannot be empty.").into_response();
    }
    if body.password.is_empty() {
        return (StatusCode::BAD_REQUEST, "Password cannot be empty.").into_response();
    }

    // db_create_user maps UNIQUE constraint violations to a user-facing message
    // returned as 409, and all other failures to a generic message returned as 500.
    if let Err(e) = db_create_user(&state.store, &body.username, &body.password) {
        let status = if e.contains("already taken") {
            StatusCode::CONFLICT
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        return (status, e).into_response();
    }

    match create_token(&state.store, &body.username) {
        Ok(token) => Json(TokenResponse { token }).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error.").into_response(),
    }
}

/// `POST /auth/login` — verify credentials and return a session token.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    if body.username.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "Username cannot be empty.").into_response();
    }

    let canonical = match db_verify_credentials(&state.store, &body.username, &body.password) {
        Ok(u) => u,
        Err(e) if e.contains("Invalid username or password") => {
            return (StatusCode::UNAUTHORIZED, e).into_response()
        }
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error.").into_response()
        }
    };

    match create_token(&state.store, &canonical) {
        Ok(token) => Json(TokenResponse { token }).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error.").into_response(),
    }
}

/// `POST /auth/logout` — revoke the current token.
pub async fn logout(
    State(state): State<AppState>,
    AuthUser(_username): AuthUser,
    headers: HeaderMap,
) -> impl IntoResponse {
    // AuthUser already validated the header; extract the token to revoke it.
    // Returning 500 here would be a bug (AuthUser guarantees the header is present),
    // but we handle it explicitly rather than panicking.
    let token = match headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        Some(t) => t.to_owned(),
        None => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error.").into_response()
        }
    };

    match revoke_token(&state.store, &token) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error.").into_response(),
    }
}
