#!/usr/bin/env bash

[ "${TARGET}" = '' ] && echo "❌ 'TARGET' env var not set" && exit 1

set -eou pipefail

# Enable static linking
export RUSTFLAGS="-C target-feature=+crt-static"
cargo build --release --target "$TARGET"
