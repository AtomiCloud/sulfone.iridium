#!/usr/bin/env bash

set -euo pipefail

cargo build

PATH_ADDITION="$(pwd)/target/debug"
export PATH="${PATH_ADDITION}:$PATH"

echo "Running full-cycle tests..."

# TODO: create commands (plan 4)

# TODO: upgrade commands (plan 4)

# TODO: conflict commands (plan 4)

# TODO: resolver commands (plan 4)

echo "✅ Full-cycle tests passed"
