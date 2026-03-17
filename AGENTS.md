# AGENTS.md

Guidance for AI agents working in this repository.

## Project

`ona_rust` is a CLI todo manager written in Rust. It stores data as JSON files in the user's home directory.

## Module layout

| File | Responsibility |
|---|---|
| `src/main.rs` | Entry point. Collects `argv`, calls `cli::run`, exits on error. |
| `src/lib.rs` | Re-exports the four public modules. |
| `src/cli.rs` | All command implementations (`cmd_add`, `cmd_list`, `cmd_done`, `cmd_edit`, `cmd_remove`, `cmd_category_add`, `cmd_category_list`) and the `run` / `run_with_store` dispatch functions. |
| `src/todo.rs` | `Todo` struct (id, text, done, category) and load/save helpers. |
| `src/category.rs` | `Category` enum (built-in variants + `Custom(String)`), `BUILTIN_CATEGORIES` constant, `parse_category`. |
| `src/storage.rs` | `Store` struct (holds file paths), `load_json` / `save_json` generics, `home_dir` helper. |
| `tests/integration.rs` | All tests. Uses `tempfile::TempDir` + `Store::from_dir` to avoid touching the real home directory. |

## CLI commands

```
todo add [--cat <category>] <text>
todo list
todo done <id>
todo edit <id> <new text>
todo remove <id>
todo category add <name>
todo category list
```

## Key design decisions

- **`Store` abstraction** — file paths are injected via `Store`. Production code uses `Store::default()` (home dir); tests use `Store::from_dir(tmp)`. Never hard-code paths.
- **`run_with_store`** — all CLI logic goes through this function so it can be called directly in tests without spawning a subprocess.
- **Error handling** — every public function returns `Result<_, String>`. An empty-string `Err` signals "already printed usage, just exit 1" (see `main.rs`).
- **ID assignment** — IDs are `u32`, assigned as `max(existing) + 1`. Overflow is an explicit error.
- **Categories** — four built-in variants in the enum; custom categories are stored as `Category::Custom(String)` and persisted in a separate JSON file. Case-insensitive matching, original casing preserved on storage.

## Adding a new command

1. Add a `cmd_<name>` function in `src/cli.rs` following the existing pattern.
2. Add a match arm in `run_with_store` in `src/cli.rs`.
3. Add the command to `print_usage` in `src/cli.rs`.
4. Add integration tests in `tests/integration.rs` (happy path + error cases).
5. Update the commands table in `README.md`.

## Development workflow

```bash
cargo build          # compile
cargo test           # run all tests
cargo clippy -- -D warnings   # lint (CI enforces zero warnings)
```

CI runs on push to `main` and on all PRs (`.github/workflows/ci.yml`): build → test → clippy.

## Data files (production)

| Path | Contents |
|---|---|
| `~/.todos.json` | Todo list |
| `~/.todo_categories.json` | Custom categories |
