#!/usr/bin/env bash

set -euo pipefail

cargo build

PATH_ADDITION="$(pwd)/target/debug"
export PATH="${PATH_ADDITION}:$PATH"

echo "Running local tests..."

# TODO: test commands (plan 2)

# TODO: try commands (plan 2)

# TODO: test init (plan 3)

# TODO: stress tests (plan 3)

echo "✅ Local tests passed"
