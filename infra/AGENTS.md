# infra/AGENTS.md

AWS CDK infrastructure for `todo-server` — ECS Fargate + EFS-backed SQLite on `ap-northeast-1`.

## Stack overview

| Resource | Details |
|---|---|
| VPC | 2 AZ, public + private subnets, 1 NAT gateway |
| ECR | `todo-server` repository (retains last 10 images) |
| ECS Cluster | `todo-server` (Fargate, Container Insights enabled) |
| ECS Service | `TodoServerStack-Service` — 1 task, rolling update, circuit breaker with rollback |
| Task Definition | 256 CPU / 512 MiB, port 3000 |
| EFS | Encrypted, `/data` access point (UID/GID 1000) — persists SQLite DB across deployments |
| ALB | Internet-facing, HTTP:80 → container:3000 |

## Prerequisites

- Node.js 18+
- AWS CDK v2: `npm install -g aws-cdk`
- AWS credentials configured (`aws configure` or environment variables)
- CDK bootstrapped in the target account/region (one-time, see below)

## Local deployment

### 1. Install dependencies

```bash
cd infra
npm install
```

### 2. Bootstrap CDK (first time only)

```bash
aws configure  # set AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, region=ap-northeast-1
cdk bootstrap aws://<ACCOUNT_ID>/ap-northeast-1
```

### 3. Preview changes

```bash
npm run diff
# or: cdk diff
```

### 4. Deploy the stack

```bash
npm run deploy
# or: cdk deploy --require-approval never
```

CDK outputs the following values after a successful deploy — note them for GitHub Actions setup:

| Output | Used as |
|---|---|
| `ApiUrl` | API endpoint |
| `EcrRepositoryUri` | ECR push target |
| `EcsClusterName` | `ECS_CLUSTER` in deploy workflow |
| `EcsServiceName` | `ECS_SERVICE` in deploy workflow |
| `TaskDefinitionFamily` | `TASK_DEFINITION_FAMILY` GitHub variable |

### 5. Push the initial Docker image

The stack deploys with a placeholder Amazon Linux image. Before the ECS service becomes healthy, push a real image manually:

```bash
# From the repository root
AWS_ACCOUNT_ID=$(aws sts get-caller-identity --query Account --output text)
ECR_URI="$AWS_ACCOUNT_ID.dkr.ecr.ap-northeast-1.amazonaws.com/todo-server"

aws ecr get-login-password --region ap-northeast-1 \
  | docker login --username AWS --password-stdin "$ECR_URI"

docker buildx build --platform linux/amd64 --push \
  --tag "$ECR_URI:latest" \
  .

# Force ECS to pull the new image
aws ecs update-service \
  --cluster todo-server \
  --service TodoServerStack-Service \
  --force-new-deployment \
  --region ap-northeast-1
```

### Useful CDK commands

```bash
npm run diff      # show pending infrastructure changes
npm run deploy    # deploy (no approval prompt)
npm run destroy   # tear down the stack (EFS and ECR are RETAIN — delete manually)
cdk deploy -c enableExec=true  # enable ECS Exec for debugging
```

---

## GitHub Actions deployment

Deployment is automated via `.github/workflows/deploy.yml`. It triggers on every push to `deploy` and runs three sequential jobs: **Test → Build & Push → Deploy to ECS**.

### Required secrets and variables

Configure these in **Settings → Secrets and variables → Actions** of the GitHub repository.

| Name | Type | Value |
|---|---|---|
| `AWS_ROLE_ARN` | Secret | ARN of the IAM role GitHub Actions assumes via OIDC |
| `TASK_DEFINITION_FAMILY` | Variable | Value of the `TaskDefinitionFamily` CDK output |

### One-time OIDC setup

GitHub Actions authenticates to AWS via OIDC (no long-lived credentials).

#### 1. Create the OIDC identity provider (once per AWS account)

```bash
aws iam create-open-id-connect-provider \
  --url https://token.actions.githubusercontent.com \
  --client-id-list sts.amazonaws.com \
  --thumbprint-list 6938fd4d98bab03faadb97b34396831e3780aea1
```

#### 2. Create the IAM role

Create a role with the following trust policy (replace `<ORG>` and `<REPO>`):

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
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
          "token.actions.githubusercontent.com:sub": "repo:<ORG>/<REPO>:*"
        }
      }
    }
  ]
}
```

Attach the following managed policies to the role:

- `AmazonEC2ContainerRegistryPowerUser` — ECR push
- `AmazonECS_FullAccess` — task definition update + service deploy

Set `AWS_ROLE_ARN` secret to the role's ARN.

### Workflow summary

```
push to deploy
  └─ test          cargo build / test / clippy
  └─ build-and-push
       ├─ OIDC → assume AWS_ROLE_ARN
       ├─ docker buildx build --platform linux/amd64
       └─ push to ECR with tag = git SHA + latest
  └─ deploy (environment: production)
       ├─ OIDC → assume AWS_ROLE_ARN
       ├─ download current task definition
       ├─ update container image to new SHA tag
       └─ aws-actions/amazon-ecs-deploy-task-definition
            wait-for-service-stability: true (10 min timeout)
```

### Manual re-deploy without a code change

```bash
gh workflow run deploy.yml --ref deploy
```

---

## Destroying the stack

```bash
cd infra
npm run destroy
```

`RemovalPolicy.RETAIN` is set on both ECR and EFS. Delete them manually after confirming data is no longer needed:

```bash
aws ecr delete-repository --repository-name todo-server --force --region ap-northeast-1
aws efs delete-file-system --file-system-id <FS_ID> --region ap-northeast-1
```
