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

## Deployment (AWS)

The API server runs on **ECS Fargate** (ap-northeast-1) behind an ALB. SQLite is persisted on **EFS**. Infrastructure is managed with **AWS CDK** in `infra/`.

### Prerequisites

- AWS CLI configured with sufficient permissions
- Node.js 18+ and CDK v2 (`npm install -g aws-cdk`)
- Docker

### First-time setup

**1. Bootstrap CDK** (once per AWS account/region):

```bash
cd infra
npm install
cdk bootstrap aws://<ACCOUNT_ID>/ap-northeast-1
```

**2. Deploy infrastructure**:

```bash
cdk deploy
```

Note the outputs — you'll need `EcrRepositoryUri`, `EcsClusterName`, `EcsServiceName`, and `TaskDefinitionFamily`.

**3. Configure GitHub OIDC**

Create an IAM Identity Provider for GitHub Actions in your AWS account:

```bash
# Create the OIDC provider
aws iam create-open-id-connect-provider \
  --url https://token.actions.githubusercontent.com \
  --client-id-list sts.amazonaws.com \
  --thumbprint-list 6938fd4d98bab03faadb97b34396831e3780aea1
```

Create an IAM role that GitHub Actions can assume. Trust policy (`trust-policy.json`):

```json
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Principal": {
      "Federated": "arn:aws:iam::<ACCOUNT_ID>:oidc-provider/token.actions.githubusercontent.com"
    },
    "Action": "sts:AssumeRoleWithWebIdentity",
    "Condition": {
      "StringEquals": {
        "token.actions.githubusercontent.com:aud": "sts.amazonaws.com"
      },
      "StringLike": {
        "token.actions.githubusercontent.com:sub": "repo:<GITHUB_ORG>/<REPO>:*"
      }
    }
  }]
}
```

Attach the following managed policies to the role:
- `AmazonEC2ContainerRegistryPowerUser`
- `AmazonECS_FullAccess`

**4. Set GitHub repository secrets**

| Secret | Value |
|---|---|
| `AWS_ROLE_ARN` | ARN of the IAM role created above |

### CI/CD flow

| Event | Workflow | Jobs |
|---|---|---|
| Pull Request | `ci.yml` | build → test → clippy |
| Push to `main` | `deploy.yml` | test → build & push to ECR → deploy to ECS |

Deployments use a rolling update strategy (min 100% healthy) with automatic rollback on failure.
