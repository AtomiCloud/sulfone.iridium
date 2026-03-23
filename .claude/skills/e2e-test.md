---
name: e2e-test
description: Write e2e test cases for cyanprint templates, plugins, processors, and resolvers
---

# E2E Test Case Writing Guide

## Test Types

### 1. `cyanprint test` (non-interactive, deterministic)

Best for: snapshot testing, parallel execution, CI pipelines.

**Template tests** — `e2e/<template>/test.cyan.yaml`

```yaml
tests:
  - name: <descriptive-name using answer values>
    expected:
      type: snapshot
      value:
        path: fixtures/expected/<name>
    answer_state:
      <question-id>:
        type: String|Bool|StringArray
        value: <expected-answer>
    deterministic_state:
      <state-key>: <value>
    validate:
      - test -f <file>
      - grep "expected" <file>
```

- `answer_state`: maps question IDs to expected answers
- `deterministic_state`: server-provided state that affects output (e.g., dates, counters)
- `validate`: shell commands run against the output directory after snapshot comparison
- Run with: `cyanprint test template ./e2e/<template> --parallel N`

**Plugin tests** — `e2e/<plugin>/test.cyan.yaml`

```yaml
tests:
  - name: <test-name>
    expected:
      type: snapshot
      value:
        path: ./snapshots/<name>
    input: ./inputs/<input-dir>
    config:
      <config-key>: <value>
    validate:
      - test -f <file>
```

- `input`: directory of files to process
- `config`: runtime configuration passed to the plugin

**Processor tests** — `e2e/<processor>/test.cyan.yaml`

```yaml
tests:
  - name: <test-name>
    expected:
      type: snapshot
      value:
        path: ./snapshots/<name>
    input: ./inputs/<input-dir>
    config:
      vars:
        name: 'value'
    globs:
      - glob: '**/*.txt'
        type: Template
        root: .
```

- `input`: directory of files to process
- `config.vars`: variables for Eta templating
- `config.parser.varSyntax`: custom delimiters `[['open', 'close']]`

**Resolver tests** — `e2e/<resolver>/test.cyan.yaml`

```yaml
tests:
  - name: <test-name>
    expected:
      type: snapshot
      value:
        path: ./snapshots/<name>
    config: {}
    resolver_inputs:
      - path: ./inputs/<dir>/template-a
        origin:
          template: template-a
          layer: 0
      - path: ./inputs/<dir>/template-b
        origin:
          template: template-b
          layer: 1
```

- `resolver_inputs`: multiple input directories with layer ordering
- Snapshots use `.txt` extension for text content (`.json` triggers JSON comparison)

### 2. `cyanprint try` / `cyanprint create` (interactive, via expect)

Best for: testing the interactive Q&A flow, project creation.

Uses `e2e/run-interactive.sh` helper — just provide the label, expect script, and optional snapshot path:

```bash
bash e2e/run-interactive.sh <label> <expect-script> [expected-snapshot]
```

| Arg                 | Description                                                                                              |
| ------------------- | -------------------------------------------------------------------------------------------------------- |
| `label`             | Descriptive name (e.g. `try-template2`, `create-template2`, `test-init`)                                 |
| `expect-script`     | Path to the `.exp` file (e.g. `e2e/expect/try-template2.exp`)                                            |
| `expected-snapshot` | Path to expected output — directory for `diff -r`, file for single-file `diff`. Omit to skip comparison. |

The helper:

1. Runs `expect <script>` (which spawns the command, sends answers, checks exit)
2. Extracts output path from the expect script (looks for `/tmp/e2e-...-output`)
3. Verifies output exists
4. Compares against expected snapshot (file or directory)
5. Prints `RUN`/`OK`/`FAIL`/`SKIP` status

**Expect script** — `e2e/expect/<label>.exp`

```expect
#!/usr/bin/expect -f
set timeout 120

# try template
spawn cyanprint try template ./e2e/<template> /tmp/e2e-try-<template>-output --disable-daemon-autostart

# create from registry
spawn cyanprint create cyane2e/<template> /tmp/e2e-create-<template>-output

# test init
spawn cyanprint test init ./e2e/<template> --text-seed "hello" --output /tmp/e2e-test-init-output --disable-daemon-autostart

expect -re {.*question.*}
send "answer\r"

# Password fields may ask for confirmation
expect -re {.*(confirmation|confirm|again|re-?enter).*}
send "answer\r"

# Select: press enter to accept default (first option highlighted)
expect -re {.*choice.*}
send "\r"

# MultiSelect: space to toggle, enter to confirm
expect -re {.*multi.*}
send " "
send "\r"

expect eof
```

Key patterns:

- Use `-re` for regex matching (handles ANSI escape codes)
- Password fields require a confirmation prompt handler
- Select defaults to first option — send `\r` to accept
- DateSelect defaults to `today` unless `DateQ.default` is set in the template
- `DateQ` supports `default?: Date`, `minDate?: Date`, `maxDate?: Date`
- Output path MUST match the pattern `/tmp/e2e-<label>-output` for the helper to extract it
- **Naming convention**: `try-<template>.exp`, `create-<template>.exp`, `test-init-<template>.exp`

**Example in local.sh:**

```bash
RUN_INTERACTIVE="bash e2e/run-interactive.sh"

$RUN_INTERACTIVE try-template2 e2e/expect/try-template2.exp \
  e2e/template2/fixtures/expected/hello:stocks:conservative:retirement:short-term-2-years:salary
$RUN_INTERACTIVE create-template2 e2e/expect/create-template2.exp
$RUN_INTERACTIVE test-init e2e/expect/test-init-template5.exp \
  e2e/template5/fixtures/expected/test-init.cyan.yaml
```

### 3. `cyanprint update` (LWW / conflict resolution)

Best for: testing upgrade and conflict flows.

```bash
# LWW: create both versions, update, verify no conflict markers
cyanprint create <template-a:1> /tmp/output
cyanprint create <template-b:1> /tmp/output
cyanprint update /tmp/output
grep -q "<<<<<<" /tmp/output/shared.txt && exit 1  # should NOT conflict

# Conflict: create both, manually edit, update, verify conflict markers
echo "user edit" > /tmp/output/shared.txt
cyanprint update /tmp/output
grep -q "<<<<<<" /tmp/output/shared.txt  # SHOULD conflict
```

### 4. Non-interactive try/create (no expect needed)

```bash
# Group try (needs pseudo-TTY)
script -q /dev/null cyanprint try group ./e2e/template4 /tmp/output --disable-daemon-autostart

# Non-interactive create (no Q&A, uses defaults)
cyanprint create cyane2e/template4 /tmp/output
```

## Snapshot Conventions

- Template test snapshots: `e2e/<template>/fixtures/expected/<name>/`
- Plugin/processor/resolver snapshots: `e2e/<type>/snapshots/<name>/`
- Binary files (`.lockb`, images) are skipped automatically
- `.json` files use JSON comparison — use `.txt` for line-based content
- Use `bash e2e/generate-snapshots.sh <label>` to capture output to `tmp/snapshots/`

## Date Handling

- Always use `DateQ` object form with explicit `default` to avoid flaky "today" tests:

```typescript
const date = await i.dateSelect({
  type: 'DateSelect',
  id: 'template/dateField',
  message: 'When does it start?',
  default: new Date('2026-03-13'), // fixed date
});
```
