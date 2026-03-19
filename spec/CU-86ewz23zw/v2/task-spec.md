# CU-86ewz23zw v2: Expand E2E Tests to Cover All Commands

## Problem

The current `e2e/e2e.sh` only builds and pushes artifacts to the local registry. It does not run any actual test, try, init, or create commands. We need comprehensive e2e coverage for:

- `cyanprint test template/plugin/processor/resolver` (non-interactive, already supported via `test.cyan.yaml`)
- `cyanprint try template` and `cyanprint try group` (try template is interactive via `inquire`)
- `cyanprint test init` (non-interactive DFS, but has a `Confirm("Delete backup?")` prompt)
- `cyanprint create` on a complex template and a group (template create is interactive)

Interactive commands use `inquire` (crossterm-based TUI) which cannot be piped via stdin. We use `expect` scripts to automate these.

**No Rust code changes.** All new test infrastructure is shell/expect scripts only.

## Scope

### In scope

- Split `e2e.sh` into 3 independent phases: `build`, `local`, `full`
- Add `cyanprint test` commands for template2, template5, plugin2, processor2, resolver2
- Add `cyanprint try template` with expect scripts for template2 and template5
- Add `cyanprint try group` for template4 (non-interactive, no expect needed)
- Create template6 — a template that generates a cyanprint template (nested templating), with 5 test cases, tested at high parallelism
- Add `cyanprint test init` with expect wrapper for template5
- Add `cyanprint create` with expect script for template2 and direct call for template4 group
- Add high-parallelism stress test section running all test commands simultaneously
- Add upgrade tests (`cyanprint create` + `cyanprint update`) with batch templates to verify LWW behavior
- Add conflict test: force-edit shared.txt after v1 create, then upgrade, verify git-like conflict markers
- Add resolver tests: internal (single template with gitignore merge) and cross-template (two templates with resolver-based merging)

### Out of scope

- Rust code changes to cyanprint
- Adding `--answers` CLI flag for non-interactive mode (deferred to future work)
- Modifying existing templates (template1-5)
- Modifying existing test.cyan.yaml files
- CI/CD pipeline integration
- Publishing v2 versions of batch templates or resolver templates (v2 artifacts already exist in repo)

## Requirements

### R1: Split e2e.sh into 3 independent phases

Rewrite `e2e/e2e.sh` as a dispatcher:

```
e2e.sh [build|local|full]   (no default — must specify)
  build  — build + push all artifacts to local registry (run once)
  local  — test + try commands only (no build, assumes artifacts already pushed)
  full   — create commands only (no build/try/test, assumes artifacts already pushed)
```

Each phase is independent. After `build` runs once, `local` and `full` can be run repeatedly without rebuilding.

**Files to create:**

- `e2e/build.sh` — extracted from current `e2e.sh` (the build/push logic)
- `e2e/local.sh` — test + try commands
- `e2e/full.sh` — create commands only

**File to modify:**

- `e2e/e2e.sh` — rewrite as dispatcher with `set -euo pipefail` and argument parsing

### R2: Add test commands to local.sh

Run `cyanprint test` on all artifacts that have `test.cyan.yaml`:

```bash
cyanprint test template ./e2e/template2 --parallel 4 --disable-daemon-autostart
cyanprint test template ./e2e/template5 --parallel 4 --disable-daemon-autostart
cyanprint test plugin ./e2e/plugin2 --parallel 2 --disable-daemon-autostart
cyanprint test processor ./e2e/processor2 --parallel 3 --disable-daemon-autostart
cyanprint test resolver ./e2e/resolver2 --parallel 3 --disable-daemon-autostart
```

Also add `cyanprint test template ./e2e/template6` once template6 exists (see R5).

### R3: Add try commands with expect scripts

#### R3a: `cyanprint try template` on template2 (expect)

Template2 prompts (from answer_state keys in `e2e/template2/test.cyan.yaml`):

1. `cyane2e/template1/name` — Text
2. `cyane2e/template2/riskTolerance` — Select (options include Conservative)
3. `cyane2e/template2/savingsGoal` — Select (options include Retirement)
4. `cyane2e/template2/incomeSource` — Select (options include Salary)
5. `cyane2e/template2/investmentType` — Select (options include Stocks)
6. `cyane2e/template2/investmentHorizon` — Select (options include "Short-term (< 2 years)")

**File: `e2e/expect/try-template2.exp`**

Use `expect -re` with regex patterns to match prompt messages. Send answers via `send "value\r"`. Use first test case values: "hello", Conservative, Retirement, Salary, Stocks, Short-term. After `expect eof`, verify output directory exists with `file exists`.

#### R3b: `cyanprint try template` on template5 (expect)

Template5 prompts (from `e2e/template5/test.cyan.yaml`):

1. `cyane2e/template5/projectName` — Text
2. `cyane2e/template5/apiKey` — Text
3. `cyane2e/template5/startDate` — Date
4. `cyane2e/template5/language` — Select (TypeScript)
5. `cyane2e/template5/useDocker` — Confirm (no)
6. `cyane2e/template5/features` — Checkbox/MultiSelect (logging, tracing)

MultiSelect requires special handling in expect: space to toggle, enter to confirm.

**File: `e2e/expect/try-template5.exp`**

#### R3c: `cyanprint try group` on template4 (no expect)

Template4 is a group referencing `cyane2e/template3:1`. `try group` is non-interactive — it uses `composition_operator.execute_template` with empty answers (Q&A handled via HTTP to template service, not inquire).

```bash
rm -rf /tmp/e2e-try-group-output
cyanprint try group ./e2e/template4 /tmp/e2e-try-group-output --disable-daemon-autostart
test -d /tmp/e2e-try-group-output
```

### R4: Add test init with expect wrapper

`cyanprint test init` in non-interactive mode auto-discovers branches via DFS (no `--interactive` flag). However, it always shows a single `Confirm("Delete backup?")` prompt after generating the test config.

**File: `e2e/expect/test-init-template5.exp`**

```expect
#!/usr/bin/expect -f
set timeout 180
spawn cyanprint test init ./e2e/template5 \
  --text-seed "hello" --password-seed "pass123" --date-seed "2026-03-13" \
  --output /tmp/e2e-test-init-output --disable-daemon-autostart
expect "Delete backup?" ; send "n\r"
expect eof
```

After running, verify `test.cyan.yaml` was generated in the output directory.

### R5: Create template6 — nested template generator

Create `e2e/template6` that generates a cyanprint template when executed. This tests that nested/recursive templating works correctly under high parallelism.

#### template6 design

Template6 is a template whose output is template7 — a simple cyanprint template with 1-2 questions.

Template6 questions:

1. `templateName` (Text) — name for the generated template
2. `authorName` (Text) — author name for the generated cyan.yaml

Template6 output (a valid cyanprint template directory):

- `cyan.yaml` — template metadata with templateName, authorName, build config pointing to `kirinnee` registry
- `cyan/Dockerfile` — simple bun/ts template service Dockerfile
- `cyan/index.ts` — simple template service with one Text question ("project name")
- `cyan/package.json` — bun project config
- `cyan/template/health.yaml` — health endpoint definition
- `blob.Dockerfile` — copies cyan/template/ into a blob tarball

#### template6 cyan.yaml

```yaml
username: cyane2e
name: template6
description: Template6 - Nested Template Generator
project: https://google.com
source: https://google.com
email: cyane2e@atomi.cloud
tags: []
readme: cyan/README.MD
processors: ['cyane2e/processor2']
plugins: ['cyane2e/plugin2']
templates: []
resolvers: []

build:
  registry: kirinnee
  platforms:
    - linux/amd64
  images:
    template:
      image: template6
      dockerfile: cyan/Dockerfile
      context: ./cyan
    blob:
      image: blob6
      dockerfile: blob.Dockerfile
      context: .
```

#### template6 test.cyan.yaml (5 test cases)

| #   | name     | author |
| --- | -------- | ------ |
| 1   | my-lib   | alice  |
| 2   | my-app   | bob    |
| 3   | api-svc  | carol  |
| 4   | cli-tool | dave   |
| 5   | web-app  | eve    |

Each test case provides the two Text answers via `answer_state`, and uses snapshot comparison to verify the generated template7 structure.

#### template6 test.cyan.yaml format

```yaml
tests:
  - name: my-lib:alice
    expected:
      type: snapshot
      value:
        path: fixtures/expected/my-lib:alice
    answer_state:
      cyane2e/template6/templateName:
        type: String
        value: my-lib
      cyane2e/template6/authorName:
        type: String
        value: alice
    deterministic_state: {}
    validate: []
  # ... repeat for other 4 cases
```

#### template6 directory structure

```
e2e/template6/
├── cyan.yaml
├── blob.Dockerfile
├── cyan/
│   ├── Dockerfile
│   ├── index.ts
│   ├── package.json
│   ├── README.MD
│   ├── tsconfig.json
│   └── template/
│       └── health.yaml
├── test.cyan.yaml
└── fixtures/
    └── expected/
        ├── my-lib:alice/
        ├── my-app:bob/
        ├── api-svc:carol/
        ├── cli-tool:dave/
        └── web-app:eve/
```

#### Build and test

Add to `e2e/build.sh`:

```bash
tag=$(openssl rand -hex 5)
cyanprint push --folder ./e2e/template6 template --build "$tag"
```

Add to `e2e/local.sh`:

```bash
cyanprint test template ./e2e/template6 --parallel 5 --disable-daemon-autostart
```

The initial run must use `--update-snapshots` to generate the expected snapshots:

```bash
cyanprint test template ./e2e/template6 --parallel 5 --update-snapshots --disable-daemon-autostart
```

### R6: Add create commands to full.sh

#### R6a: Create from template2 (expect — has Q&A)

Same prompt flow as `try template` on template2 (name, riskTolerance, savingsGoal, incomeSource, investmentType, investmentHorizon).

**File: `e2e/expect/create-template2.exp`**

```expect
#!/usr/bin/expect -f
set timeout 120
spawn cyanprint create cyane2e/template2 /tmp/e2e-create-t2-output
# Same prompts as try-template2
expect -re ".*name.*" ; send "hello\r"
expect -re ".*risk.*" ; send "\r"
# ... etc
expect eof
# Verify
```

#### R6b: Create from template4 group (no expect — no Q&A)

```bash
rm -rf /tmp/e2e-create-group-output
cyanprint create cyane2e/template4 /tmp/e2e-create-group-output
test -d /tmp/e2e-create-group-output
```

### R7: High parallelism stress tests

Add a section to `e2e/local.sh` that runs ALL test commands simultaneously as background processes, then waits and checks exit codes. This stresses port allocation to verify the race condition fix works under load.

```bash
echo "Running high-parallelism stress tests..."
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

if [ -n "$FAIL" ]; then echo "Stress tests FAILED"; exit 1; fi
echo "All stress tests passed"
```

### R8: Upgrade tests (batch layering + LWW)

Verify that `cyanprint create` followed by `cyanprint update` correctly handles batch layering with Last-Writer-Wins (LWW) conflict resolution. Uses the existing `test-batch-a` and `test-batch-b` templates.

#### R8a: Batch create — LWW verification (no conflict)

Create a project from two templates that share `shared.txt`, then verify LWW applies (last written template wins, no conflict markers):

```bash
rm -rf /tmp/e2e-upgrade-lww
# Create from test-batch-a:v1 first (writes shared.txt = "hello from A v1")
cyanprint create cyane2e/test-batch-a:1 /tmp/e2e-upgrade-lww
# Create from test-batch-b:v1 into same directory (writes shared.txt = "hello from B v1")
cyanprint create cyane2e/test-batch-b:1 /tmp/e2e-upgrade-lww

# Verify: shared.txt should contain "hello from B v1" (LWW — B was last writer)
test "$(cat /tmp/e2e-upgrade-lww/shared.txt)" = "hello from B v1"
# Verify: unique files from both templates exist
test -f /tmp/e2e-upgrade-lww/a-only.txt
test -f /tmp/e2e-upgrade-lww/b-only.txt
# Verify: NO conflict markers in shared.txt
! grep -q "<<<<<<" /tmp/e2e-upgrade-lww/shared.txt
```

**Note:** These templates have no interactive Q&A (they use processor1 which is deterministic), so no expect script is needed.

#### R8b: Batch update — version upgrade with LWW

After creating the v1 batch, push v2 versions and run `cyanprint update` to verify upgrade behavior:

```bash
rm -rf /tmp/e2e-upgrade-update
# Create initial project with v1 templates
cyanprint create cyane2e/test-batch-a:1 /tmp/e2e-upgrade-update
cyanprint create cyane2e/test-batch-b:1 /tmp/e2e-upgrade-update

# v2 artifacts already exist in local registry (test-batch-a-v2, test-batch-b-v2)
# Run update to upgrade all templates to v2
cyanprint update /tmp/e2e-upgrade-update

# Verify: shared.txt should now reflect v2 content (LWW between A v2 and B v2)
# A v2 shared.txt = "hello from A v2 - CHANGED", B v2 shared.txt = "hello from B v2 - CHANGED"
# Last writer wins — verify it's one of the v2 values
grep -q "v2" /tmp/e2e-upgrade-update/shared.txt
# Verify: NO conflict markers
! grep -q "<<<<<<" /tmp/e2e-upgrade-update/shared.txt
# Verify: unique files updated
grep -q "v2" /tmp/e2e-upgrade-update/a-only.txt
grep -q "v2" /tmp/e2e-upgrade-update/b-only.txt
```

**Add to `e2e/full.sh`.**

### R9: Conflict test — forced edit + upgrade

Verify that `cyanprint update` detects git-like conflicts when a user has manually edited a file that templates also modify. The conflict markers (`<<<<<<<`/`=======`/`>>>>>>>`) should appear in the conflicting file.

#### R9a: Force edit shared.txt after v1 create, then upgrade

```bash
rm -rf /tmp/e2e-upgrade-conflict
# Create initial project with v1 templates
cyanprint create cyane2e/test-batch-a:1 /tmp/e2e-upgrade-conflict
cyanprint create cyane2e/test-batch-b:1 /tmp/e2e-upgrade-conflict

# Force-edit shared.txt to simulate user changes
echo "USER EDIT: do not overwrite" > /tmp/e2e-upgrade-conflict/shared.txt

# Run update — should detect conflict on shared.txt since both v2 templates also modify it
cyanprint update /tmp/e2e-upgrade-conflict

# Verify: conflict markers exist in shared.txt
grep -q "<<<<<<" /tmp/e2e-upgrade-conflict/shared.txt
grep -q "=======" /tmp/e2e-upgrade-conflict/shared.txt
grep -q ">>>>>>>" /tmp/e2e-upgrade-conflict/shared.txt
# Verify: the user edit is preserved in the conflict
grep -q "USER EDIT" /tmp/e2e-upgrade-conflict/shared.txt
# Verify: exactly 1 conflict (shared.txt only — a-only.txt and b-only.txt are LWW, no user edit)
CONFLICT_COUNT=$(grep -c "<<<<<<" /tmp/e2e-upgrade-conflict/shared.txt)
test "$CONFLICT_COUNT" -eq 1
```

**Add to `e2e/full.sh`.**

### R10: Resolver tests

Verify that resolvers correctly merge files within and across templates. Uses the existing `template-resolver-1-v1` and `template-resolver-2-v1` templates.

#### R10a: Internal resolver test — single template with internal gitignore merge

`template-resolver-1-v1` has both `internal/.gitignore` and `template/.gitignore`, configured with `cyane2e/resolver2:1` for `.gitignore` file. The resolver should merge the two gitignore files.

```bash
rm -rf /tmp/e2e-resolver-internal
cyanprint create cyane2e/template-resolver-1:1 /tmp/e2e-resolver-internal

# Verify: .gitignore exists and contains content from BOTH internal/ and template/
test -f /tmp/e2e-resolver-internal/.gitignore
# Internal .gitignore contains "ignore-type-1", template .gitignore contains "internal"
# The resolver (resolver2:1) should merge them (line-based merger)
grep -q "ignore-type-1" /tmp/e2e-resolver-internal/.gitignore
grep -q "internal" /tmp/e2e-resolver-internal/.gitignore
```

#### R10b: Cross-template resolver test — two templates with resolver-based merging

Create from both `template-resolver-1-v1` and `template-resolver-2-v1` into the same directory. Multiple files should be merged via their configured resolvers:

- `c1.json` — both use `resolver1:1` with `arrayStrategy: concat`
- `c2.json` — both use `resolver1:1` with `arrayStrategy: replace`
- `c3.json` — both use `resolver1:1` with `arrayStrategy: distinct`
- `.gitignore` — both use `resolver2:1` (line merger)
- `package.json` — only resolver-1 configures `resolver1:1` with `arrayStrategy: distinct`
- `force_conflict.txt` — NO resolver configured on either → git-like conflict markers expected
- `from-1.txt` / `from-2.txt` — unique to each template (no conflict)

```bash
rm -rf /tmp/e2e-resolver-cross
cyanprint create cyane2e/template-resolver-1:1 /tmp/e2e-resolver-cross
cyanprint create cyane2e/template-resolver-2:1 /tmp/e2e-resolver-cross

# Verify: both unique files exist
test -f /tmp/e2e-resolver-cross/from-1.txt
test -f /tmp/e2e-resolver-cross/from-2.txt

# Verify: gitignore merged (both have resolver2:1 for .gitignore)
test -f /tmp/e2e-resolver-cross/.gitignore
grep -q "ignore-type-1" /tmp/e2e-resolver-cross/.gitignore
grep -q "ignore-type-2" /tmp/e2e-resolver-cross/.gitignore

# Verify: c1.json merged with concat strategy (arrays combined)
test -f /tmp/e2e-resolver-cross/c1.json

# Verify: c2.json merged with replace strategy (last wins)
test -f /tmp/e2e-resolver-cross/c2.json

# Verify: c3.json merged with distinct strategy (deduplicated)
test -f /tmp/e2e-resolver-cross/c3.json

# Verify: package.json merged (resolver-1 has distinct strategy for it)
test -f /tmp/e2e-resolver-cross/package.json

# Verify: force_conflict.txt has git-like conflict markers (NO resolver configured)
grep -q "<<<<<<" /tmp/e2e-resolver-cross/force_conflict.txt
grep -q "conflict from 1" /tmp/e2e-resolver-cross/force_conflict.txt
grep -q "conflict from 2" /tmp/e2e-resolver-cross/force_conflict.txt
```

**Add to `e2e/full.sh`.**

## Non-Goals

- No Rust code changes to cyanprint
- No `--answers` CLI flag (deferred)
- No CI/CD pipeline changes
- No modifications to existing templates or test.cyan.yaml files (template1-5, plugin2, processor2, resolver2)

## Implementation Order

1. Split `e2e.sh` into `build.sh`, `local.sh`, `full.sh` with dispatcher
2. Create template6 (nested template generator) — cyan.yaml, template service source, Dockerfiles
3. Add template6 push to `build.sh`
4. Run template6 test with `--update-snapshots` to generate initial expected fixtures
5. Add all `test` commands to `local.sh` (template2, template5, template6, plugin2, processor2, resolver2)
6. Add `try` commands to `local.sh` — expect scripts for template2/template5, direct call for template4 group
7. Add `test init` expect wrapper to `local.sh`
8. Add parallel stress test section to `local.sh`
9. Add `create` commands to `full.sh` — expect script for template2, direct call for template4 group
10. Add upgrade tests to `full.sh` — batch create (LWW), batch update (v1→v2 upgrade)
11. Add conflict test to `full.sh` — force-edit shared.txt, upgrade, verify conflict markers
12. Add resolver tests to `full.sh` — internal gitignore merge, cross-template resolver merging

## Files to Create/Modify

### New files:

- `e2e/build.sh`
- `e2e/local.sh`
- `e2e/full.sh`
- `e2e/expect/try-template2.exp`
- `e2e/expect/try-template5.exp`
- `e2e/expect/test-init-template5.exp`
- `e2e/expect/create-template2.exp`
- `e2e/template6/cyan.yaml`
- `e2e/template6/cyan/Dockerfile`
- `e2e/template6/cyan/index.ts`
- `e2e/template6/cyan/package.json`
- `e2e/template6/cyan/tsconfig.json`
- `e2e/template6/cyan/README.MD`
- `e2e/template6/cyan/template/health.yaml`
- `e2e/template6/blob.Dockerfile`
- `e2e/template6/test.cyan.yaml`

### Modified files:

- `e2e/e2e.sh` — rewrite as dispatcher

## Verification

1. `./e2e.sh build` — all artifacts publish to local registry (run once)
2. `./e2e.sh local` — all test + try commands pass, parallel stress test passes (re-runnable without rebuild)
3. `./e2e.sh full` — all create, upgrade, conflict, and resolver commands pass (re-runnable without rebuild)
4. No port conflicts in parallel stress test runs
5. Upgrade tests verify LWW behavior (no conflict markers on shared files)
6. Conflict test verifies git-like conflict markers on force-edited files
7. Resolver tests verify internal gitignore merge and cross-template file merging
