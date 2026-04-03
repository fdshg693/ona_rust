use crate::api::state::AppState;
use crate::auth::validate_token;
use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};

/// Axum extractor that validates the `Authorization: Bearer <token>` header
/// and resolves it to the authenticated username.
pub struct AuthUser(pub String);

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header"))?;

        let token = header
            .strip_prefix("Bearer ")
            .ok_or((StatusCode::UNAUTHORIZED, "Authorization header must use Bearer scheme"))?;

        validate_token(&state.store, token)
            .map(AuthUser)
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid or expired token"))
    }
}
