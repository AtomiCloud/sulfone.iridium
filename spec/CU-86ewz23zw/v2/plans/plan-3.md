# Plan 3: Populate local.sh with test, try, test init, and stress tests

## Spec requirements

R2: Add test commands to local.sh
R3: Add try commands with expect scripts
R4: Add test init with expect wrapper
R7: High parallelism stress tests

## Overview

Fill in the skeleton `e2e/local.sh` with all test commands, expect scripts for interactive try/test-init commands, and a parallel stress test section. Create the `e2e/expect/` directory with all expect scripts.

## Steps

### 1. Create `e2e/expect/` directory

### 2. Create `e2e/expect/try-template2.exp`

Expect script for `cyanprint try template ./e2e/template2 /tmp/e2e-try-template2-output --disable-daemon-autostart`.

6 prompts to answer using `expect -re` with regex patterns:

1. `.*name.*` → send "hello\r"
2. `.*risk.*` → send "\r" (default = Conservative, first option)
3. `.*savings.*` → send "\r" (default = Retirement)
4. `.*income.*` → send "\r" (default = Salary)
5. `.*investment.*type.*` → send "\r" (default = Stocks)
6. `.*horizon.*` → send "\r" (default = Short-term)

After `expect eof`, verify output directory exists. Timeout: 120s.

### 3. Create `e2e/expect/try-template5.exp`

Expect script for `cyanprint try template ./e2e/template5 /tmp/e2e-try-template5-output --disable-daemon-autostart`.

6 prompts:

1. `.*project.*name.*` → send "my-project\r" (Text)
2. `.*api.*key.*` → send "sk-test-123\r" (Text)
3. `.*start.*date.*` → send "2026-01-15\r" (Date)
4. `.*language.*` → send "\r" (Select, default = TypeScript)
5. `.*docker.*` → send "n\r" (Confirm)
6. `.*features.*` → MultiSelect: send space to toggle first option (logging), send space for second (tracing), send "\r" to confirm

After `expect eof`, verify output directory exists. Timeout: 120s.

### 4. Create `e2e/expect/test-init-template5.exp`

Expect script for `cyanprint test init ./e2e/template5 --text-seed "hello" --password-seed "pass123" --date-seed "2026-03-13" --output /tmp/e2e-test-init-output --disable-daemon-autostart`.

Single prompt:

1. `Delete backup?` → send "n\r"

After `expect eof`, verify test.cyan.yaml exists in output dir. Timeout: 180s.

### 5. Populate `e2e/local.sh` — test commands section

Add the following `cyanprint test` commands:

```bash
echo "🔍 Running test commands..."
cyanprint test template ./e2e/template2 --parallel 4 --disable-daemon-autostart
cyanprint test template ./e2e/template5 --parallel 4 --disable-daemon-autostart
cyanprint test template ./e2e/template6 --parallel 5 --disable-daemon-autostart
cyanprint test plugin ./e2e/plugin2 --parallel 2 --disable-daemon-autostart
cyanprint test processor ./e2e/processor2 --parallel 3 --disable-daemon-autostart
cyanprint test resolver ./e2e/resolver2 --parallel 3 --disable-daemon-autostart
```

### 6. Populate `e2e/local.sh` — try commands section

```bash
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
```

### 7. Populate `e2e/local.sh` — test init section

```bash
echo "🔍 Running test init..."
rm -rf /tmp/e2e-test-init-output
expect e2e/expect/test-init-template5.exp
test -f /tmp/e2e-test-init-output/test.cyan.yaml
```

### 8. Populate `e2e/local.sh` — stress test section

Run all test commands simultaneously as background processes, wait for all, check exit codes:

```bash
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

if [ -n "$FAIL" ]; then echo "❌ Stress tests FAILED"; exit 1; fi
echo "✅ All stress tests passed"
```

## Files

| Action | File                                  |
| ------ | ------------------------------------- |
| Create | `e2e/expect/try-template2.exp`        |
| Create | `e2e/expect/try-template5.exp`        |
| Create | `e2e/expect/test-init-template5.exp`  |
| Modify | `e2e/local.sh` (fill in all sections) |

## No Rust code changes
