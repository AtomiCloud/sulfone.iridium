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

echo "🧪 Running full-cycle tests..."

# --- create commands ---
echo "🔍 Running create commands..."

rm -rf /tmp/e2e-create-t2-output
./e2e/run-interactive.sh create-template2 e2e/expect/create-template2.exp

# create from template4 group (template2 dep has interactive questions)
echo "▶️  RUN create-group (create)"
rm -rf /tmp/e2e-create-group-output
./e2e/run-interactive.sh create-group e2e/expect/create-group-template4.exp

# --- upgrade tests (LWW) ---
echo "🔍 Running upgrade tests (LWW)..."

# batch create — LWW verification (no conflict)
echo "▶️  RUN upgrade-lww (create + create, verify LWW)"
rm -rf /tmp/e2e-upgrade-lww
cyanprint create cyane2e/test-batch-a:1 /tmp/e2e-upgrade-lww
cyanprint create cyane2e/test-batch-b:1 /tmp/e2e-upgrade-lww
test "$(cat /tmp/e2e-upgrade-lww/shared.txt)" = "hello from B v1"
test -f /tmp/e2e-upgrade-lww/a-only.txt
test -f /tmp/e2e-upgrade-lww/b-only.txt
grep -q "<<<<<<" /tmp/e2e-upgrade-lww/shared.txt && exit 1
echo "✅ OK upgrade-lww"

# batch update — version upgrade with LWW
echo "▶️  RUN upgrade-update (create + update, verify v2)"
rm -rf /tmp/e2e-upgrade-update
cyanprint create cyane2e/test-batch-a:1 /tmp/e2e-upgrade-update
cyanprint create cyane2e/test-batch-b:1 /tmp/e2e-upgrade-update
cyanprint update /tmp/e2e-upgrade-update
grep -q "v2" /tmp/e2e-upgrade-update/shared.txt
grep -q "<<<<<<" /tmp/e2e-upgrade-update/shared.txt && exit 1
grep -q "v2" /tmp/e2e-upgrade-update/a-only.txt
echo "✅ OK upgrade-update"

# --- conflict test ---
echo "🔍 Running conflict test..."
echo "▶️  RUN upgrade-conflict (create + edit + update, verify conflict markers)"
rm -rf /tmp/e2e-upgrade-conflict
cyanprint create cyane2e/test-batch-a:1 /tmp/e2e-upgrade-conflict
cyanprint create cyane2e/test-batch-b:1 /tmp/e2e-upgrade-conflict
echo "USER EDIT: do not overwrite" >/tmp/e2e-upgrade-conflict/shared.txt
cyanprint update /tmp/e2e-upgrade-conflict
grep -q "<<<<<<" /tmp/e2e-upgrade-conflict/shared.txt
grep -q "=======" /tmp/e2e-upgrade-conflict/shared.txt
grep -q ">>>>>>>" /tmp/e2e-upgrade-conflict/shared.txt
grep -q "USER EDIT" /tmp/e2e-upgrade-conflict/shared.txt
CONFLICT_COUNT=$(grep -c "<<<<<<" /tmp/e2e-upgrade-conflict/shared.txt)
test "$CONFLICT_COUNT" -eq 1
echo "✅ OK upgrade-conflict"

# --- resolver tests ---
echo "🔍 Running resolver tests..."

# internal resolver test
echo "▶️  RUN resolver-internal (create, verify .gitignore merge)"
rm -rf /tmp/e2e-resolver-internal
cyanprint create cyane2e/template-resolver-1:1 /tmp/e2e-resolver-internal
test -f /tmp/e2e-resolver-internal/.gitignore
grep -q "ignore-type-1" /tmp/e2e-resolver-internal/.gitignore
grep -q "internal" /tmp/e2e-resolver-internal/.gitignore
echo "✅ OK resolver-internal"

# cross-template resolver test
echo "▶️  RUN resolver-cross (create + create, verify cross-template merge + conflict)"
rm -rf /tmp/e2e-resolver-cross
cyanprint create cyane2e/template-resolver-1:1 /tmp/e2e-resolver-cross
cyanprint create cyane2e/template-resolver-2:1 /tmp/e2e-resolver-cross
test -f /tmp/e2e-resolver-cross/from-1.txt
test -f /tmp/e2e-resolver-cross/from-2.txt
test -f /tmp/e2e-resolver-cross/.gitignore
grep -q "ignore-type-1" /tmp/e2e-resolver-cross/.gitignore
grep -q "ignore-type-2" /tmp/e2e-resolver-cross/.gitignore
test -f /tmp/e2e-resolver-cross/c1.json
test -f /tmp/e2e-resolver-cross/c2.json
test -f /tmp/e2e-resolver-cross/c3.json
test -f /tmp/e2e-resolver-cross/package.json
# force_conflict.txt has NO resolver — LWW picks last writer, no conflict markers
grep -q "conflict from 2" /tmp/e2e-resolver-cross/force_conflict.txt
grep -q "<<<<<<" /tmp/e2e-resolver-cross/force_conflict.txt && exit 1
echo "✅ OK resolver-cross"

echo "✅ Full-cycle tests passed"
