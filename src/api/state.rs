use crate::storage::Store;

/// Shared application state injected into every Axum handler.
#[derive(Clone)]
pub struct AppState {
    pub store: Store,
}
