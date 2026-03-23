#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Source .env if it exists
ENV_FILE="${SCRIPT_DIR}/../.env"
if [ -f "$ENV_FILE" ]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

cargo build

PATH_ADDITION="$(pwd)/target/debug"
export PATH="${PATH_ADDITION}:$PATH"

echo "🧪 Running local tests..."

# --- test commands ---
echo "🔍 Running test commands..."
echo "▶️  RUN template2 (test)"
cyanprint test template ./e2e/template2 --disable-daemon-autostart
echo "▶️  RUN template5 (test)"
cyanprint test template ./e2e/template5 --disable-daemon-autostart
echo "▶️  RUN template6 (test)"
cyanprint test template ./e2e/template6 --disable-daemon-autostart
echo "▶️  RUN plugin2 (test)"
cyanprint test plugin ./e2e/plugin2 --disable-daemon-autostart
echo "▶️  RUN processor2 (test)"
cyanprint test processor ./e2e/processor2 --disable-daemon-autostart
echo "▶️  RUN resolver2 (test)"
cyanprint test resolver ./e2e/resolver2 --disable-daemon-autostart

# --- try commands ---
echo "🔍 Running try commands..."
./e2e/run-interactive.sh try-template2 e2e/expect/try-template2.exp \
  e2e/template2/fixtures/expected/hello:stocks:conservative:retirement:short-term-2-years:salary
./e2e/run-interactive.sh try-template5 e2e/expect/try-template5.exp \
  e2e/template5/fixtures/expected/hello:pass123:2026-03-13:typescript:no:logging

# try template4 group — skipped: group try is interactive (member templates have questions)
# TODO: add back when `try group` supports non-interactive mode

# --- test init ---
echo "🔍 Running test init..."
# Restore the original test.cyan.yaml before test init overwrites it
git checkout -- ./e2e/template5/test.cyan.yaml 2>/dev/null || true
./e2e/run-interactive.sh test-init e2e/expect/test-init-template5.exp

# --- stress tests ---
echo "🔬 Running high-parallelism stress tests..."
echo "▶️  RUN template2 (parallel 4)"
cyanprint test template ./e2e/template2 --parallel 4 --disable-daemon-autostart &
PID_T2=$!
echo "▶️  RUN template5 (parallel 4)"
cyanprint test template ./e2e/template5 --parallel 4 --disable-daemon-autostart &
PID_T5=$!
echo "▶️  RUN template6 (parallel 5)"
cyanprint test template ./e2e/template6 --parallel 5 --disable-daemon-autostart &
PID_T6=$!
echo "▶️  RUN processor2 (parallel 3)"
cyanprint test processor ./e2e/processor2 --parallel 3 --disable-daemon-autostart &
PID_P2=$!
echo "▶️  RUN plugin2 (parallel 2)"
cyanprint test plugin ./e2e/plugin2 --parallel 2 --disable-daemon-autostart &
PID_PL=$!
echo "▶️  RUN resolver2 (parallel 3)"
cyanprint test resolver ./e2e/resolver2 --parallel 3 --disable-daemon-autostart &
PID_R2=$!

FAIL=""
wait $PID_T2 || FAIL=1
wait $PID_T5 || FAIL=1
wait $PID_T6 || FAIL=1
wait $PID_P2 || FAIL=1
wait $PID_PL || FAIL=1
wait $PID_R2 || FAIL=1

if [ -n "$FAIL" ]; then
  echo "❌ Stress tests FAILED"
  exit 1
fi
echo "✅ All stress tests passed"

echo "✅ Local tests passed"
