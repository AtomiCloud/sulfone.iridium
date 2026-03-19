#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  echo "Usage: $0 <build|local|full>"
  echo ""
  echo "  build  - Build and publish all e2e test artifacts"
  echo "  local  - Run local tests (requires build artifacts)"
  echo "  full   - Run full-cycle tests (requires build artifacts)"
  exit 1
}

if [[ $# -ne 1 ]]; then
  usage
fi

case "$1" in
build)
  exec "$SCRIPT_DIR/build.sh"
  ;;
local)
  exec "$SCRIPT_DIR/local.sh"
  ;;
full)
  exec "$SCRIPT_DIR/full.sh"
  ;;
*)
  echo "Error: unknown phase '$1'"
  usage
  ;;
esac
