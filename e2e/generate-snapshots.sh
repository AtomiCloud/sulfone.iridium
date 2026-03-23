#!/usr/bin/env bash

# Snapshot generation for try/init outputs
# Run after running local.sh (or individual try commands) to capture expected snapshots
# Usage: bash e2e/generate-snapshots.sh [all|try-template2|try-template5|test-init|try-group]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="${SCRIPT_DIR}/.."
TMP_DIR="${BASE_DIR}/tmp/snapshots"

mkdir -p "$TMP_DIR"

generate() {
  local name="$1"
  local src="$2"
  local dst="$TMP_DIR/$name"

  rm -rf "$dst"

  if [ ! -d "$src" ]; then
    echo "  SKIP $name: $src does not exist (run the try command first)"
    return 0
  fi
  cp -r "$src" "$dst"
  # Remove .lockb binary files from snapshots
  find "$dst" -name "*.lockb" -delete
  echo "  OK $name: captured from $src"
}

case "${1:-all}" in
try-template2)
  generate try-template2 /tmp/e2e-try-template2-output
  ;;
try-template5)
  generate try-template5 /tmp/e2e-try-template5-output
  ;;
try-group)
  generate try-group /tmp/e2e-try-group-output
  ;;
test-init)
  # Only capture test.cyan.yaml, not the full template output
  rm -rf "$TMP_DIR/test-init"
  mkdir -p "$TMP_DIR/test-init"
  if [ -f /tmp/e2e-test-init-output/test.cyan.yaml ]; then
    cp /tmp/e2e-test-init-output/test.cyan.yaml "$TMP_DIR/test-init/"
    echo "  OK test-init: captured test.cyan.yaml"
  else
    echo "  SKIP test-init: /tmp/e2e-test-init-output/test.cyan.yaml does not exist"
  fi
  ;;
all)
  echo "Generating snapshots to $TMP_DIR ..."
  generate try-template2 /tmp/e2e-try-template2-output
  generate try-template5 /tmp/e2e-try-template5-output
  generate try-group /tmp/e2e-try-group-output
  # test-init
  rm -rf "$TMP_DIR/test-init"
  mkdir -p "$TMP_DIR/test-init"
  if [ -f /tmp/e2e-test-init-output/test.cyan.yaml ]; then
    cp /tmp/e2e-test-init-output/test.cyan.yaml "$TMP_DIR/test-init/"
    echo "  OK test-init: captured test.cyan.yaml"
  else
    echo "  SKIP test-init: /tmp/e2e-test-init-output/test.cyan.yaml does not exist"
  fi
  echo "Done. Review snapshots in $TMP_DIR then copy to the appropriate fixtures/expected/ directories."
  ;;
*)
  echo "Usage: $0 [all|try-template2|try-template5|test-init|try-group]"
  exit 1
  ;;
esac
