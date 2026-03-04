# Plan 1: E2E Setup Script

## Goal

Create `e2e/setup.sh` and wire it as `e2e:setup` in Taskfile.yaml. Single plan — small, focused scope.

## Files to Modify

| File            | Action | Description                                                                            |
| --------------- | ------ | -------------------------------------------------------------------------------------- |
| `e2e/setup.sh`  | CREATE | Bash script: auth flow, user setup, token creation, .env generation, cyan.yaml updates |
| `Taskfile.yaml` | MODIFY | Add `e2e:setup` task wrapping script with `infisical run --env lapras --`              |

## Approach

### `e2e/setup.sh`

Follow existing e2e script conventions (`set -eou pipefail`, env var checks with `❌` messages).

**Script flow:**

1. Validate required env vars (`DESCOPE_PROJECT`, `DESCOPE_TOKEN`) — these come from infisical injection
2. Construct `DESCOPE_AUTH="${DESCOPE_PROJECT}:${DESCOPE_TOKEN}"`
3. Generate OTP via Descope management API (`/v1/mgmt/tests/generate/otp`)
4. Parse OTP code from response with `jq`
5. Verify OTP via Descope auth API (`/v1/auth/otp/verify/email`) → extract session JWT as `AUTH`
6. Check if user `P2Wskb04HSJQRfckShfhtWXwUiUd` exists (GET, check HTTP status)
7. If 404, create user with username `cyane2e`
8. Create CYAN_TOKEN via POST to `/User/{USER_ID}/tokens`
9. Write `.env` file with all values (`DOCKER_USERNAME=` left blank)
10. Find all `cyan.yaml` under `e2e/` and set `username: cyane2e` using `sed`

**Error handling:** Use `curl -sf` or check HTTP response codes. Fail fast on any step failure thanks to `set -e`.

**Idempotency:** User creation is guarded by existence check. Token creation generates a new token each run (acceptable per spec).

### `Taskfile.yaml`

Add under existing tasks:

```yaml
e2e:setup:
  desc: 'Setup e2e environment (auth token, user, .env)'
  cmds:
    - infisical run --env lapras -- ./e2e/setup.sh
```

Placed before the existing `e2e` task logically (setup runs before tests).

## Edge Cases

- Descope OTP response format: parse carefully with `jq`, fail if fields missing
- User already exists: 200 response → skip creation (idempotent)
- Token endpoint response: extract the actual token value (check API response structure)
- `cyan.yaml` files may have varying indentation — use `sed` pattern that handles this

## Testing

- Run `task e2e:setup` with services running locally
- Verify `.env` is written with correct values
- Verify all `e2e/**/cyan.yaml` have `username: cyane2e`
- Run `task e2e` after setup to confirm full flow works

## Implementation Checklist

- [ ] Create `e2e/setup.sh`
  - [ ] Load DESCOPE secrets from env (injected by infisical)
  - [ ] Generate OTP and verify to get AUTH token
  - [ ] Check if user exists; create if not
  - [ ] Create CYAN_TOKEN
  - [ ] Write `.env` file
  - [ ] Update `username` in all `e2e/**/cyan.yaml` files
- [ ] Make `e2e/setup.sh` executable (`chmod +x`)
- [ ] Add `e2e:setup` task to `Taskfile.yaml`
