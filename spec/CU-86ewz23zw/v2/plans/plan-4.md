# Plan 4: Populate full.sh with create, upgrade, conflict, and resolver tests

## Spec requirements

R6: Add create commands to full.sh
R8: Upgrade tests (batch layering + LWW)
R9: Conflict test (forced edit + upgrade)
R10: Resolver tests (internal + cross-template)

## Overview

Fill in the skeleton `e2e/full.sh` with create commands (interactive expect + non-interactive), upgrade tests that verify LWW behavior, a conflict test with force-edited files, and resolver tests for internal and cross-template merging.

## Steps

### 1. Create `e2e/expect/create-template2.exp`

Expect script for `cyanprint create cyane2e/template2 /tmp/e2e-create-t2-output`.

Same 6 prompts as try-template2 (name, riskTolerance, savingsGoal, incomeSource, investmentType, investmentHorizon), same answers:

1. `.*name.*` → "hello\r"
2. `.*risk.*` → "\r"
3. `.*savings.*` → "\r"
4. `.*income.*` → "\r"
5. `.*investment.*type.*` → "\r"
6. `.*horizon.*` → "\r"

After `expect eof`, verify output directory exists. Timeout: 120s.

### 2. Populate `e2e/full.sh` — create commands section

#### Create from template2 (interactive — expect)

```bash
echo "🔍 Running create commands..."
rm -rf /tmp/e2e-create-t2-output
expect e2e/expect/create-template2.exp
test -d /tmp/e2e-create-t2-output
```

#### Create from template4 group (non-interactive)

```bash
rm -rf /tmp/e2e-create-group-output
cyanprint create cyane2e/template4 /tmp/e2e-create-group-output
test -d /tmp/e2e-create-group-output
```

### 3. Populate `e2e/full.sh` — upgrade tests section (R8)

#### R8a: Batch create — LWW verification (no conflict)

```bash
echo "🔍 Running upgrade tests (LWW)..."
rm -rf /tmp/e2e-upgrade-lww
cyanprint create cyane2e/test-batch-a:1 /tmp/e2e-upgrade-lww
cyanprint create cyane2e/test-batch-b:1 /tmp/e2e-upgrade-lww
test "$(cat /tmp/e2e-upgrade-lww/shared.txt)" = "hello from B v1"
test -f /tmp/e2e-upgrade-lww/a-only.txt
test -f /tmp/e2e-upgrade-lww/b-only.txt
! grep -q "<<<<<<" /tmp/e2e-upgrade-lww/shared.txt
```

#### R8b: Batch update — version upgrade with LWW

```bash
rm -rf /tmp/e2e-upgrade-update
cyanprint create cyane2e/test-batch-a:1 /tmp/e2e-upgrade-update
cyanprint create cyane2e/test-batch-b:1 /tmp/e2e-upgrade-update
cyanprint update /tmp/e2e-upgrade-update
grep -q "v2" /tmp/e2e-upgrade-update/shared.txt
! grep -q "<<<<<<" /tmp/e2e-upgrade-update/shared.txt
grep -q "v2" /tmp/e2e-upgrade-update/a-only.txt
grep -q "v2" /tmp/e2e-upgrade-update/b-only.txt
```

### 4. Populate `e2e/full.sh` — conflict test section (R9)

```bash
echo "🔍 Running conflict test..."
rm -rf /tmp/e2e-upgrade-conflict
cyanprint create cyane2e/test-batch-a:1 /tmp/e2e-upgrade-conflict
cyanprint create cyane2e/test-batch-b:1 /tmp/e2e-upgrade-conflict
echo "USER EDIT: do not overwrite" > /tmp/e2e-upgrade-conflict/shared.txt
cyanprint update /tmp/e2e-upgrade-conflict
grep -q "<<<<<<" /tmp/e2e-upgrade-conflict/shared.txt
grep -q "=======" /tmp/e2e-upgrade-conflict/shared.txt
grep -q ">>>>>>>" /tmp/e2e-upgrade-conflict/shared.txt
grep -q "USER EDIT" /tmp/e2e-upgrade-conflict/shared.txt
CONFLICT_COUNT=$(grep -c "<<<<<<" /tmp/e2e-upgrade-conflict/shared.txt)
test "$CONFLICT_COUNT" -eq 1
```

### 5. Populate `e2e/full.sh` — resolver tests section (R10)

#### R10a: Internal resolver test

```bash
echo "🔍 Running resolver tests..."
rm -rf /tmp/e2e-resolver-internal
cyanprint create cyane2e/template-resolver-1:1 /tmp/e2e-resolver-internal
test -f /tmp/e2e-resolver-internal/.gitignore
grep -q "ignore-type-1" /tmp/e2e-resolver-internal/.gitignore
grep -q "internal" /tmp/e2e-resolver-internal/.gitignore
```

#### R10b: Cross-template resolver test

```bash
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
```

## Files

| Action | File                                 |
| ------ | ------------------------------------ |
| Create | `e2e/expect/create-template2.exp`    |
| Modify | `e2e/full.sh` (fill in all sections) |

## No Rust code changes
