# AGENTS.md

Guidance for AI agents working in this repository.

## Project

`ona_rust` is a CLI todo manager written in Rust. It stores data in a SQLite database in the user's home directory. It also ships a REST API server (`todo-server`) built with Axum.

## Module layout

| File | Responsibility |
|---|---|
| `src/main.rs` | CLI entry point. Collects `argv`, calls `cli::run`, exits on error. |
| `src/api_main.rs` | REST server entry point. Binds on `PORT` (default 3000) and serves the Axum router. |
| `src/lib.rs` | Re-exports all public modules. |
| `src/cli.rs` | All CLI command implementations and the `run` / `run_with_store` dispatch functions. |
| `src/auth.rs` | `cmd_register`, `cmd_login`, `cmd_logout`, `require_auth` (CLI session file). Also `create_token`, `validate_token`, `revoke_token` (REST bearer tokens). Passwords hashed with bcrypt. |
| `src/todo.rs` | `Todo` struct (id, text, done, category) and load/save helpers. |
| `src/category.rs` | `Category` enum (built-in variants + `Custom(String)`), `BUILTIN_CATEGORIES` constant, `parse_category`. |
| `src/storage.rs` | `Store` struct (holds the DB path), `Store::open()` opens a `rusqlite::Connection` and creates the schema on first use. |
| `src/api/mod.rs` | `build_router` — assembles the Axum `Router` with all routes and CORS middleware. |
| `src/api/state.rs` | `AppState` — shared state injected into every handler (holds `Store`). |
| `src/api/extractor.rs` | `AuthUser` — Axum extractor that validates `Authorization: Bearer <token>`. |
| `src/api/auth_handlers.rs` | REST handlers: `POST /auth/register`, `POST /auth/login`, `POST /auth/logout`. |
| `src/api/todo_handlers.rs` | REST handlers for todos and categories. |
| `tests/integration.rs` | CLI integration tests. Uses `tempfile::TempDir` + `Store::from_dir`. |
| `tests/api_integration.rs` | REST API integration tests. Uses `tower::ServiceExt::oneshot` against the in-process router. |

## CLI commands

```
todo register <username> <password>
todo login <username> <password>
todo logout
todo add [--cat <category>] <text>
todo list [--page <n>]
todo done <id>
todo edit <id> <new text>
todo remove <id>
todo category add <name>
todo category list
todo category edit <name> <new name>
todo category remove <name>
```

`register`, `login`, and `logout` do not require an active session. All other commands require a valid session (created by `login` or `register`).

`todo list` without `--page` launches an interactive pager (arrow keys to navigate, `q`/`Esc`/`Enter` to exit). `PAGE_SIZE` is 10.

## Key design decisions

- **`Store` abstraction** — the DB path is injected via `Store`. Production code uses `Store::new()` (home dir); tests use `Store::from_dir(tmp)`. Never hard-code paths.
- **`Store::open()`** — every load/save call opens a connection via `Store::open()`, which runs `CREATE TABLE IF NOT EXISTS` for `users`, `todos`, `categories`, and `sessions` on first use. No separate migration step is needed.
- **`run_with_store`** — all CLI logic goes through this function so it can be called directly in tests without spawning a subprocess.
- **Error handling** — every public function returns `Result<_, String>`. An empty-string `Err` signals "already printed usage, just exit 1" (see `main.rs`).
- **ID assignment** — IDs are `u32`, assigned as `max(existing) + 1`. Overflow is an explicit error.
- **Categories** — four built-in variants in the enum; custom categories are stored as `Category::Custom(String)` and persisted in the `categories` table. Stored as their display string (e.g. `"work"`, `"hobby"`). Case-insensitive matching, original casing preserved on storage. Built-in categories cannot be added, edited, or removed.
- **CLI authentication** — passwords are hashed with bcrypt (`DEFAULT_COST`). The active session is stored as a plain username in `~/.todo_session`. In tests, use `set_session_path_for_test(path)` (thread-local) to avoid parallel-test interference.
- **REST authentication** — `POST /auth/login` (or `/auth/register`) returns an opaque UUID token stored in the `sessions` table with a 24-hour TTL. Clients send it as `Authorization: Bearer <token>`. The `AuthUser` extractor validates it on every protected route. `POST /auth/logout` deletes the token row.
- **REST API tests** — use `tower::ServiceExt::oneshot` to drive the in-process Axum router. Each test creates its own `TempDir` + `Store` so tests are fully isolated.

## Development workflow

```bash
cargo build                    # compile
cargo test                     # run all tests
cargo test <test_name>         # run a single test by name
cargo test category            # run all tests whose name contains "category"
cargo clippy -- -D warnings    # lint (CI enforces zero warnings)
```

CI runs on push to `main` and on all PRs (`.github/workflows/ci.yml`): build → test → clippy.

## PR requirements

- Branch off `main`; do not commit directly to `main`.
- CI must pass (build + test + clippy with zero warnings) before merging.
- Follow the existing commit style: `<type>: <short description>` (e.g. `feat: add category remove command`).

## Adding a new command

1. Add a `cmd_<name>` function in `src/cli.rs` following the existing pattern.
2. Add a match arm in `run_with_store` in `src/cli.rs`.
3. Add the command to `print_usage` in `src/cli.rs`.
4. Add integration tests in `tests/integration.rs` (happy path + error cases).
5. Update the commands table in this file and in `README.md`.

## Adding a new REST endpoint

1. Add a handler function in `src/api/auth_handlers.rs` or `src/api/todo_handlers.rs`.
2. Register the route in `build_router` in `src/api/mod.rs`.
3. Add integration tests in `tests/api_integration.rs` (happy path + error cases).

## Data files (production)

| Path | Contents |
|---|---|
| `~/.todos.db` | SQLite database — `users`, `todos`, `categories`, and `sessions` tables |
| `~/.todo_session` | Plain-text file containing the logged-in username (CLI only) |
