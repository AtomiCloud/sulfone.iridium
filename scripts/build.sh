#!/usr/bin/env bash

[ "${TARGET}" = '' ] && echo "❌ 'TARGET' env var not set" && exit 1
[ "${BIN_NAME}" = '' ] && echo "❌ 'BIN_NAME' env var not set" && exit 1

set -eou pipefail

# Enable static linking
nix build .#

mkdir -p "target/${TARGET}/release"
cp "./result/bin/${BIN_NAME}" "target/${TARGET}/release/${BIN_NAME}"
