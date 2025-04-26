#!/usr/bin/env bash

[ "${RUST_VERSION}" = '' ] && echo "❌ 'RUST_VERSION' env var not set" && exit 1

set -eou pipefail

goreleaser release --clean --snapshot
