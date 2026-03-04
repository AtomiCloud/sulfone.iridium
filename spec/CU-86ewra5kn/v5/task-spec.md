# CU-86ewra5kn v5: E2E Setup Script (e2e:setup Taskfile task)

## Context

v4 completed the `TemplateSpecManager` refactoring (composable DI-style API). PR #67 is open with that work.

This v5 spec is a new task on top of v4: add an `e2e:setup` script that:

1. Obtains an auth token from Descope via OTP flow
2. Ensures the `cyane2e` user exists in the registry (creates it if not)
3. Creates a `CYAN_TOKEN` for that user
4. Writes the `.env` file for e2e tests
5. Updates all `cyan.yaml` files in the `e2e/` directory to set `username: cyane2e`

## Goal

Add `e2e:setup` as a Taskfile task backed by a bash script `e2e/setup.sh`. This script should be idempotent: safe to run repeatedly.

## Constants

- `USER_ID`: `P2Wskb04HSJQRfckShfhtWXwUiUd` (fixed)
- `USER_NAME`: `cyane2e` (pinned)
- `CYANPRINT_REGISTRY`: `http://localhost:9001`
- `CYANPRINT_COORDINATOR`: `http://localhost:9000`

## Environment Variables (from Infisical)

- `DESCOPE_PROJECT`: Descope project ID
- `DESCOPE_TOKEN`: Descope management token
- `DESCOPE_AUTH`: `${DESCOPE_PROJECT}:${DESCOPE_TOKEN}` (constructed in script)

Infisical environments: `local` (lapras) and `sulfone`.

Look at existing scripts (e.g. `e2e/publish-template.sh`) to understand how infisical is used in this project.

## Auth Flow

### Step 1: Generate OTP

```bash
curl --request POST \
  --url https://api.descope.com/v1/mgmt/tests/generate/otp \
  --header "authorization: Bearer ${DESCOPE_AUTH}" \
  --header 'content-type: application/json' \
  --header "x-descope-project-id: ${DESCOPE_PROJECT}" \
  --data '{
    "loginId": "test1",
    "deliveryMethod": "email"
  }'
```

Response contains the OTP code (parse from JSON).

### Step 2: Verify OTP → get AUTH token

```bash
curl --request POST \
  --url https://api.descope.com/v1/auth/otp/verify/email \
  --header "authorization: Bearer ${DESCOPE_AUTH}" \
  --header 'content-type: application/json' \
  --data '{
    "loginId": "test1",
    "code": "'"${OTP}"'"
  }'
```

Response contains the Bearer token (parse `sessionJwt` or similar field). This is `AUTH` from here on.

### Step 3: Check if user exists

```bash
curl --request GET \
  --url "http://localhost:9001/api/v1/User/${USER_ID}" \
  --header "authorization: Bearer ${AUTH}"
```

- HTTP 200: user exists, skip creation
- HTTP 404: create user (Step 4)

### Step 4: Create user (if not exists)

```bash
curl --request POST \
  --url http://localhost:9001/api/v1/User \
  --header "authorization: Bearer ${AUTH}" \
  --header 'content-type: application/json' \
  --data '{
    "username": "'"${USER_NAME}"'"
  }'
```

### Step 5: Create CYAN_TOKEN

```bash
curl --request POST \
  --url "http://localhost:9001/api/v1/User/${USER_ID}/tokens" \
  --header "authorization: Bearer ${AUTH}" \
  --header 'content-type: application/json' \
  --data '{"name": "e2e-token"}'
```

Parse the token value from the response JSON.

## .env Output

Write `.env` at the repo root:

```
CYANPRINT_USERNAME=cyane2e
CYANPRINT_REGISTRY=http://localhost:9001
CYANPRINT_COORDINATOR=http://localhost:9000
CYAN_TOKEN=<token from Step 5>
DOCKER_USERNAME=
```

`DOCKER_USERNAME` is intentionally left blank for the user to fill in.

## Update cyan.yaml files

For every `cyan.yaml` found recursively under `e2e/` (any depth), set `username: cyane2e`.

Use `find` + `sed` (or `yq` if available) to update the `username` field. The existing cyan.yaml files already have `username: cyane2e` but the script should enforce it idempotently.

## Files to Modify / Create

### 1. `e2e/setup.sh` (NEW)

A bash script implementing all steps above. Key conventions:

- `#!/usr/bin/env bash`
- `set -eou pipefail`
- No binary-existence checks (nix provides all tools)
- Use `jq` for JSON parsing
- Use `curl` for HTTP requests
- Infisical secret loading: `infisical run --env lapras -- <command>` (wraps the script execution, secrets injected as env vars)

### 2. `Taskfile.yaml` (MODIFY)

Add task:

```yaml
e2e:setup:
  desc: 'Setup e2e environment (auth token, user, .env)'
  cmds:
    - infisical run --env lapras -- ./e2e/setup.sh
```

Note: `e2e:setup` is separate from `e2e` (which runs the tests). The setup task must be run first.

## Acceptance Criteria

1. `task e2e:setup` runs without error when services are up
2. `.env` is created with correct values (except `DOCKER_USERNAME` which is left blank)
3. `CYAN_TOKEN` is a valid token for `cyane2e` user
4. All `cyan.yaml` files under `e2e/` have `username: cyane2e`
5. Script is idempotent: running it again does not fail (creates new token each run is acceptable)
6. Script does not check for binary existence (nix handles this)
7. Taskfile task wraps with `infisical run --env lapras --` so secrets (`DESCOPE_PROJECT`, `DESCOPE_TOKEN`) are injected as env vars

## Checklist

### Code

- [ ] Create `e2e/setup.sh`
  - [ ] Load DESCOPE secrets from infisical
  - [ ] Generate OTP and verify to get AUTH token
  - [ ] Check if user exists; create if not
  - [ ] Create CYAN_TOKEN
  - [ ] Write `.env` file
  - [ ] Update `username` in all `e2e/**/cyan.yaml` files
- [ ] Make `e2e/setup.sh` executable
- [ ] Add `e2e:setup` task to `Taskfile.yaml`
