#!/usr/bin/env bash

# E2E Setup Script
# Sets up the e2e environment by:
# 1. Authenticating with Descope via OTP
# 2. Validating AUTH token via /Me endpoint
# 3. Creating/verifying the e2e user in local registry
# 4. Generating a CYAN_TOKEN from local registry
# 5. Detecting Docker username (cross-platform)
# 6. Writing .env file
# 7. Updating cyan.yaml files

# Validate required env vars (injected by infisical)
[ "$DESCOPE_PROJECT" = '' ] && echo "❌ 'DESCOPE_PROJECT' env var not set" && exit 1
[ "$DESCOPE_TOKEN" = '' ] && echo "❌ 'DESCOPE_TOKEN' env var not set" && exit 1

set -eou pipefail

# Constants
USERNAME="cyane2e"
LOGIN_ID="test1"
DESCOPE_API="https://api.descope.com"
LOCAL_REGISTRY="http://localhost:9001"
# DESCOPE_AUTH is ONLY for Descope management endpoints (steps 1-2)
DESCOPE_AUTH="${DESCOPE_PROJECT}:${DESCOPE_TOKEN}"

echo "🔍 Setting up e2e environment..."

# Step 1: Generate OTP via Descope management API
echo "🔍 Generating OTP..."
OTP_RESPONSE=$(curl -sf -X POST \
  "${DESCOPE_API}/v1/mgmt/tests/generate/otp" \
  -H "Authorization: Bearer ${DESCOPE_AUTH}" \
  -H "Content-Type: application/json" \
  -H "x-descope-project-id: ${DESCOPE_PROJECT}" \
  -d "{\"loginId\": \"${LOGIN_ID}\", \"deliveryMethod\": \"email\"}")

# Parse OTP code from response
OTP_CODE=$(echo "$OTP_RESPONSE" | jq -r '.code // empty')
[ "$OTP_CODE" = '' ] && echo "❌ Failed to parse OTP code from response" && exit 1
echo "✅ OTP generated: $OTP_CODE"

# Step 2: Verify OTP via Descope auth API to get session JWT
echo "🔍 Verifying OTP..."
AUTH_RESPONSE=$(curl -sf -X POST \
  "${DESCOPE_API}/v1/auth/otp/verify/email" \
  -H "Authorization: Bearer ${DESCOPE_AUTH}" \
  -H "Content-Type: application/json" \
  -d "{\"loginId\": \"${LOGIN_ID}\", \"code\": \"${OTP_CODE}\"}")

# Extract session JWT - this is the AUTH token for localhost:9001 API calls
AUTH=$(echo "$AUTH_RESPONSE" | jq -r '.sessionJwt // empty')
[ "$AUTH" = '' ] && echo "❌ Failed to parse session JWT from response" && exit 1
echo "✅ Auth token obtained"

# Step 3: Validate AUTH token and get user ID from /Me endpoint
echo "🔍 Validating AUTH token and getting user ID..."
USER_ID=$(curl -sf \
  "${LOCAL_REGISTRY}/api/v1/User/Me" \
  -H "Authorization: Bearer ${AUTH}")

[ "$USER_ID" = '' ] && echo "❌ Failed to get user ID from /Me response" && exit 1
echo "✅ AUTH token validated, user ID: $USER_ID"

# Step 4: Check if user exists in local registry using AUTH token
echo "🔍 Checking if user exists in local registry..."
USER_STATUS=$(curl -s -o /dev/null -w "%{http_code}" \
  "${LOCAL_REGISTRY}/api/v1/User/${USER_ID}" \
  -H "Authorization: Bearer ${AUTH}")

if [ "$USER_STATUS" = "200" ]; then
  echo "✅ User already exists in local registry"
elif [ "$USER_STATUS" = "401" ]; then
  # Since AUTH is validated, 401 means user not found - create user
  echo "🔍 User not found, creating in local registry..."
  curl -sf -X POST \
    --url "${LOCAL_REGISTRY}/api/v1/User" \
    -H "Authorization: Bearer ${AUTH}" \
    -H "Content-Type: application/json" \
    -d "{\"username\": \"${USERNAME}\"}"
  echo "✅ User created in local registry"
else
  echo "❌ Unexpected status code when checking user: $USER_STATUS"
  exit 1
fi

# Step 5: Create CYAN_TOKEN via local registry API using AUTH token
echo "🔍 Creating CYAN_TOKEN..."
TOKEN_RESPONSE=$(curl -sf -X POST \
  --url "${LOCAL_REGISTRY}/api/v1/User/${USER_ID}/tokens" \
  -H "Authorization: Bearer ${AUTH}" \
  -H "Content-Type: application/json" \
  -d '{"name": "e2e-token"}')

CYAN_TOKEN=$(echo "$TOKEN_RESPONSE" | jq -r '.apiKey // empty')
[ "$CYAN_TOKEN" = '' ] && echo "❌ Failed to parse token from response" && exit 1
echo "✅ CYAN_TOKEN created"

# Step 6: Get Docker username (cross-platform)
echo "🔍 Detecting Docker username..."
get_docker_username() {
  local config_file="$HOME/.docker/config.json"
  local registry="https://index.docker.io/v1/"

  if [ -f "$config_file" ]; then
    local creds_store
    creds_store=$(jq -r '.credsStore // empty' "$config_file" 2>/dev/null)

    if [ -n "$creds_store" ]; then
      # Use credential helper (osxkeychain, pass, wincred, etc.)
      echo "$registry" | "docker-credential-$creds_store" get 2>/dev/null | jq -r '.Username // empty'
    else
      # Fallback: decode auth field directly
      local auth
      auth=$(jq -r '.auths["'"$registry"'"].auth // empty' "$config_file" 2>/dev/null)
      if [ -n "$auth" ]; then
        echo "$auth" | base64 -d 2>/dev/null | cut -d: -f1
      fi
    fi
  fi
}
DOCKER_USERNAME=$(get_docker_username)
[ "$DOCKER_USERNAME" = '' ] && DOCKER_USERNAME=""
echo "✅ Docker username: ${DOCKER_USERNAME:-<not detected>}"

# Step 7: Write .env file
echo "🔍 Writing .env file..."
cat >.env <<EOF
CYANPRINT_USERNAME=${USERNAME}
CYANPRINT_REGISTRY=http://localhost:9001
CYANPRINT_COORDINATOR=http://localhost:9000
CYAN_TOKEN=${CYAN_TOKEN}
DOCKER_USERNAME=${DOCKER_USERNAME}
EOF
echo "✅ .env file written"

echo "✅ E2E setup complete!"
