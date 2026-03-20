#!/usr/bin/env bash

set -euo pipefail

cargo build

PATH_ADDITION="$(pwd)/target/debug"
export PATH="${PATH_ADDITION}:$PATH"

echo "Running full-cycle tests..."

# --- create commands ---
echo "🔍 Running create commands..."

# create from template2 (interactive — expect)
rm -rf /tmp/e2e-create-t2-output
expect e2e/expect/create-template2.exp
test -d /tmp/e2e-create-t2-output

# create from template4 group (non-interactive)
rm -rf /tmp/e2e-create-group-output
cyanprint create cyane2e/template4 /tmp/e2e-create-group-output
test -d /tmp/e2e-create-group-output

# --- upgrade tests (LWW) ---
echo "🔍 Running upgrade tests (LWW)..."

# batch create — LWW verification (no conflict)
rm -rf /tmp/e2e-upgrade-lww
cyanprint create cyane2e/test-batch-a:1 /tmp/e2e-upgrade-lww
cyanprint create cyane2e/test-batch-b:1 /tmp/e2e-upgrade-lww
test "$(cat /tmp/e2e-upgrade-lww/shared.txt)" = "hello from B v1"
test -f /tmp/e2e-upgrade-lww/a-only.txt
test -f /tmp/e2e-upgrade-lww/b-only.txt
grep -q "<<<<<<" /tmp/e2e-upgrade-lww/shared.txt && exit 1

# batch update — version upgrade with LWW
rm -rf /tmp/e2e-upgrade-update
cyanprint create cyane2e/test-batch-a:1 /tmp/e2e-upgrade-update
cyanprint create cyane2e/test-batch-b:1 /tmp/e2e-upgrade-update
cyanprint update /tmp/e2e-upgrade-update
grep -q "v2" /tmp/e2e-upgrade-update/shared.txt
grep -q "<<<<<<" /tmp/e2e-upgrade-update/shared.txt && exit 1
grep -q "v2" /tmp/e2e-upgrade-update/a-only.txt
grep -q "v2" /tmp/e2e-upgrade-update/b-only.txt

# --- conflict test ---
echo "🔍 Running conflict test..."
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

# --- resolver tests ---
echo "🔍 Running resolver tests..."

# internal resolver test
rm -rf /tmp/e2e-resolver-internal
cyanprint create cyane2e/template-resolver-1:1 /tmp/e2e-resolver-internal
test -f /tmp/e2e-resolver-internal/.gitignore
grep -q "ignore-type-1" /tmp/e2e-resolver-internal/.gitignore
grep -q "internal" /tmp/e2e-resolver-internal/.gitignore

# cross-template resolver test
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
grep -q "<<<<<<" /tmp/e2e-resolver-cross/force_conflict.txt
grep -q "conflict from 1" /tmp/e2e-resolver-cross/force_conflict.txt
grep -q "conflict from 2" /tmp/e2e-resolver-cross/force_conflict.txt

echo "✅ Full-cycle tests passed"
