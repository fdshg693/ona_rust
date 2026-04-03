# AGENTS.md

Guidance for AI agents working in this repository.

## Project

`ona_rust` is a CLI todo manager written in Rust. It stores data in a SQLite database in the user's home directory.

## Module layout

| File | Responsibility |
|---|---|
| `src/main.rs` | Entry point. Collects `argv`, calls `cli::run`, exits on error. |
| `src/lib.rs` | Re-exports the five public modules. |
| `src/cli.rs` | All command implementations (`cmd_add`, `cmd_list`, `cmd_list_interactive`, `cmd_done`, `cmd_edit`, `cmd_remove`, `cmd_category_add`, `cmd_category_list`, `cmd_category_edit`, `cmd_category_remove`) and the `run` / `run_with_store` dispatch functions. |
| `src/auth.rs` | `cmd_register`, `cmd_login`, `cmd_logout`, `require_auth`. Passwords hashed with bcrypt. Session stored in `~/.todo_session`. |
| `src/todo.rs` | `Todo` struct (id, text, done, category) and load/save helpers. |
| `src/category.rs` | `Category` enum (built-in variants + `Custom(String)`), `BUILTIN_CATEGORIES` constant, `parse_category`. |
| `src/storage.rs` | `Store` struct (holds the DB path), `Store::open()` opens a `rusqlite::Connection` and creates the schema on first use, `home_dir` helper. |
| `tests/integration.rs` | All tests. Uses `tempfile::TempDir` + `Store::from_dir` to avoid touching the real home directory. |

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

- **`Store` abstraction** — the DB path is injected via `Store`. Production code uses `Store::default()` (home dir); tests use `Store::from_dir(tmp)`. Never hard-code paths.
- **`Store::open()`** — every load/save call opens a connection via `Store::open()`, which runs `CREATE TABLE IF NOT EXISTS` for `users`, `todos`, and `categories` on first use. No separate migration step is needed.
- **`run_with_store`** — all CLI logic goes through this function so it can be called directly in tests without spawning a subprocess.
- **Error handling** — every public function returns `Result<_, String>`. An empty-string `Err` signals "already printed usage, just exit 1" (see `main.rs`).
- **ID assignment** — IDs are `u32`, assigned as `max(existing) + 1`. Overflow is an explicit error.
- **Categories** — four built-in variants in the enum; custom categories are stored as `Category::Custom(String)` and persisted in the `categories` table. Stored as their display string (e.g. `"work"`, `"hobby"`). Case-insensitive matching, original casing preserved on storage. Built-in categories cannot be added, edited, or removed.
- **Authentication** — passwords are hashed with bcrypt (`DEFAULT_COST`). The active session is stored as a plain username in `~/.todo_session`. `run_with_store` calls `require_auth()` before dispatching any todo command; `register`, `login`, and `logout` are matched first and bypass the gate. In tests, use `set_session_path_for_test(path)` (thread-local) instead of env vars to avoid parallel-test interference.

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

## Data files (production)

| Path | Contents |
|---|---|
| `~/.todos.db` | SQLite database — `users`, `todos`, and `categories` tables |
| `~/.todo_session` | Plain-text file containing the logged-in username |
