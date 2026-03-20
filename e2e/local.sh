#!/usr/bin/env bash

set -euo pipefail

cargo build

PATH_ADDITION="$(pwd)/target/debug"
export PATH="${PATH_ADDITION}:$PATH"

echo "Running local tests..."

# --- test commands ---
echo "🔍 Running test commands..."
cyanprint test template ./e2e/template2 --parallel 4 --disable-daemon-autostart
cyanprint test template ./e2e/template5 --parallel 4 --disable-daemon-autostart
cyanprint test template ./e2e/template6 --parallel 5 --disable-daemon-autostart
cyanprint test plugin ./e2e/plugin2 --parallel 2 --disable-daemon-autostart
cyanprint test processor ./e2e/processor2 --parallel 3 --disable-daemon-autostart
cyanprint test resolver ./e2e/resolver2 --parallel 3 --disable-daemon-autostart

# --- try commands ---
echo "🔍 Running try commands..."

# try template2 (interactive — expect)
expect e2e/expect/try-template2.exp
test -d /tmp/e2e-try-template2-output

# try template5 (interactive — expect)
expect e2e/expect/try-template5.exp
test -d /tmp/e2e-try-template5-output

# try template4 group (non-interactive — no expect)
rm -rf /tmp/e2e-try-group-output
cyanprint try group ./e2e/template4 /tmp/e2e-try-group-output --disable-daemon-autostart
test -d /tmp/e2e-try-group-output

# --- test init ---
echo "🔍 Running test init..."
rm -rf /tmp/e2e-test-init-output
expect e2e/expect/test-init-template5.exp
test -f /tmp/e2e-test-init-output/test.cyan.yaml

# --- stress tests ---
echo "🔬 Running high-parallelism stress tests..."
cyanprint test template ./e2e/template2 --parallel 4 --disable-daemon-autostart &
PID_T2=$!
cyanprint test template ./e2e/template5 --parallel 4 --disable-daemon-autostart &
PID_T5=$!
cyanprint test template ./e2e/template6 --parallel 5 --disable-daemon-autostart &
PID_T6=$!
cyanprint test processor ./e2e/processor2 --parallel 3 --disable-daemon-autostart &
PID_P2=$!
cyanprint test plugin ./e2e/plugin2 --parallel 2 --disable-daemon-autostart &
PID_PL=$!
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
