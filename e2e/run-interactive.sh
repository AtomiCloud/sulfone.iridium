#!/usr/bin/env bash

# Run an interactive e2e test (try or create) via expect, then compare output to snapshot.
#
# Usage:
#   bash e2e/run-interactive.sh <label> <expect-script> <expected-snapshot-dir>
#   bash e2e/run-interactive.sh <label> <expect-script>               # snapshot-only: skip comparison
#
# Examples:
#   bash e2e/run-interactive.sh try-template2 e2e/expect/try-template2.exp e2e/template2/fixtures/expected/hello:stocks:conservative:retirement:short-term-2-years:salary
#   bash e2e/run-interactive.sh test-init      e2e/expect/test-init-template5.exp  e2e/template5/fixtures/expected/test-init.cyan.yaml

set -euo pipefail

LABEL="$1"
EXP_SCRIPT="$2"
EXPECTED="${3:-}"

if [ ! -f "$EXP_SCRIPT" ]; then
  echo "❌ FAIL $LABEL: expect script not found: $EXP_SCRIPT"
  exit 1
fi

echo "▶️  RUN $LABEL ($EXP_SCRIPT)"

# Run the expect script — it handles spawn, answers, and basic exit checks
expect "$EXP_SCRIPT"

# Extract the output path from the expect script (look for the last /tmp/e2e-...-output)
OUTPUT_DIR="$(grep -oE '/tmp/e2e-[a-zA-Z0-9_-]+-output' "$EXP_SCRIPT" | tail -1)"

# Snapshot comparison (optional)
if [ -z "$EXPECTED" ]; then
  # No snapshot to compare — expect script's exit code is the authority
  echo "✅ OK $LABEL: expect script passed"
  exit 0
fi

if [ ! -e "$EXPECTED" ]; then
  echo "⏭️  SKIP $LABEL: no expected snapshot at $EXPECTED"
  echo "    Run: bash e2e/generate-snapshots.sh $LABEL"
  exit 0
fi

# Handle file vs directory comparison
if [ -f "$EXPECTED" ]; then
  # Single file comparison
  ACTUAL="$OUTPUT_DIR/$(basename "$EXPECTED")"
  if [ ! -f "$ACTUAL" ]; then
    echo "❌ FAIL $LABEL: expected file $(basename "$EXPECTED") not found in output"
    exit 1
  fi
  DIFF_OUTPUT=$(diff --exclude='*.lockb' --exclude='.DS_Store' "$ACTUAL" "$EXPECTED" 2>&1) || true
else
  # Directory comparison
  if [ ! -d "$OUTPUT_DIR" ]; then
    echo "❌ FAIL $LABEL: output is not a directory at $OUTPUT_DIR"
    exit 1
  fi
  DIFF_OUTPUT=$(diff -r --exclude='*.lockb' --exclude='.DS_Store' "$OUTPUT_DIR" "$EXPECTED" 2>&1) || true
fi

if [ -z "$DIFF_OUTPUT" ]; then
  echo "✅ OK $LABEL: snapshot matched"
else
  echo "❌ FAIL $LABEL: snapshot mismatch"
  echo "$DIFF_OUTPUT"
  exit 1
fi
