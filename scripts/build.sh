#!/usr/bin/env bash

[ "${TARGET}" = '' ] && echo "❌ 'TARGET' env var not set" && exit 1

set -eou pipefail

cargo build --release --target "$TARGET"
