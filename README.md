# ona_rust

A command-line Todo list manager with user authentication. Supports adding, completing, and removing tasks, with optional category tagging. Data is persisted in a SQLite database in the home directory.

## Build

```bash
cargo build --release
```

The binary is output to `target/release/todo`.

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

## Data Files

| File | Contents |
|---|---|
| `~/.todos.db` | SQLite database — `users`, `todos`, and `categories` tables |
| `~/.todo_session` | Plain-text file containing the logged-in username |
