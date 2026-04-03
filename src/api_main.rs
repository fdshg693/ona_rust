use ona_rust::api::{build_router, state::AppState};
use ona_rust::storage::Store;
use std::env;

#[tokio::main]
async fn main() {
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let store = Store::new().unwrap_or_else(|e| {
        eprintln!("Failed to initialise store: {e}");
        std::process::exit(1);
    });

    let state = AppState { store };
    let router = build_router(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap_or_else(|e| {
        eprintln!("Failed to bind {addr}: {e}");
        std::process::exit(1);
    });

    println!("todo-server listening on {addr}");
    axum::serve(listener, router).await.unwrap_or_else(|e| {
        eprintln!("Server error: {e}");
        std::process::exit(1);
    });
}
