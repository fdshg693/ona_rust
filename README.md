# ona_rust

A command-line Todo list manager with user authentication. Supports adding, completing, and removing tasks, with optional category tagging. Data is persisted in a SQLite database in the home directory.

Also ships a REST API server (`todo-server`) for external access.

## Build

```bash
cargo build --release
```

Binaries are output to `target/release/`:
- `todo` — CLI
- `todo-server` — REST API server

## Usage

```
todo <command> [args]
```

### Commands

**Auth (no login required)**

| Command | Description |
|---|---|
| `register <username> <password>` | Create an account and log in |
| `login <username> <password>` | Log in to an existing account |
| `logout` | End the current session |

**Todo (login required)**

| Command | Description |
|---|---|
| `add <text>` | Add a new todo |
| `add --cat <category> <text>` | Add a todo with a category |
| `list` | List todos interactively (←/→ to page) |
| `list --page <n>` | Print a specific page non-interactively |
| `done <id>` | Mark a todo as done |
| `edit <id> <new text>` | Update the text of a todo |
| `remove <id>` | Remove a todo |
| `category add <name>` | Add a custom category |
| `category edit <name> <new name>` | Rename a custom category |
| `category remove <name>` | Remove a custom category |
| `category list` | List available categories |

### Examples

```bash
# Create an account
todo register alice mypassword

# Add a todo
todo add Buy milk

# Add a todo with a category
todo add --cat shopping Buy milk

# List todos
todo list
# [ ] #1 [shopping]: Buy milk

# Mark as done
todo done 1

# Edit a todo's text
todo edit 1 Buy oat milk

# Remove a todo
todo remove 1

# Add a custom category
todo category add hobby

# List categories
todo category list

# Log out
todo logout

# Log back in
todo login alice mypassword
```

## Categories

Built-in categories: `work`, `personal`, `shopping`, `health`. Use `category add` to define custom ones.

## REST API

Start the server:

```bash
todo-server           # listens on :3000 by default
PORT=8080 todo-server # custom port
```

All todo and category endpoints require `Authorization: Bearer <token>`. Obtain a token via `/auth/register` or `/auth/login`.

### Endpoints

**Auth**

| Method | Path | Description |
|---|---|---|
| `POST` | `/auth/register` | Create account, returns `{ "token": "..." }` |
| `POST` | `/auth/login` | Verify credentials, returns `{ "token": "..." }` |
| `POST` | `/auth/logout` | Revoke current token |

**Todos** *(require Bearer token)*

| Method | Path | Description |
|---|---|---|
| `GET` | `/todos?page=<n>` | List todos (10 per page, default page 1) |
| `POST` | `/todos` | Add a todo — body: `{ "text": "...", "category": "..." }` |
| `PUT` | `/todos/:id` | Edit todo text — body: `{ "text": "..." }` |
| `PATCH` | `/todos/:id/done` | Mark todo as done |
| `DELETE` | `/todos/:id` | Remove a todo |

**Categories** *(require Bearer token)*

| Method | Path | Description |
|---|---|---|
| `GET` | `/categories` | List built-in and custom categories |
| `POST` | `/categories` | Add custom category — body: `{ "name": "..." }` |
| `PUT` | `/categories/:name` | Rename category — body: `{ "new_name": "..." }` |
| `DELETE` | `/categories/:name` | Remove custom category |

### Example

```bash
# Register
curl -s -X POST http://localhost:3000/auth/register \
  -H 'Content-Type: application/json' \
  -d '{"username":"alice","password":"secret"}' | jq .
# { "token": "550e8400-..." }

TOKEN=550e8400-...

# Add a todo
curl -s -X POST http://localhost:3000/todos \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"text":"Buy milk","category":"shopping"}' | jq .

# List todos
curl -s http://localhost:3000/todos \
  -H "Authorization: Bearer $TOKEN" | jq .
```

## Data Files

| File | Contents |
|---|---|
| `~/.todos.db` | SQLite database — `users`, `todos`, `categories`, and `sessions` tables |
| `~/.todo_session` | Plain-text file containing the logged-in username (CLI only) |
