#!/usr/bin/env bash

[ "${TARGET}" = '' ] && echo "❌ 'TARGET' env var not set" && exit 1
[ "${BIN_NAME}" = '' ] && echo "❌ 'BIN_NAME' env var not set" && exit 1

cache="$1"

set -eou pipefail

# Enable static linking

TO_PUSH=$(nix build .# --print-out-paths)
echo "🔍 Built $TO_PUSH"

mkdir -p "target/${TARGET}/release"
cp "./result/bin/${BIN_NAME}" "target/${TARGET}/release/${BIN_NAME}"

if [ "$cache" != "" ]; then
  echo "🫸 Pushing all shells to Attic $cache"
  # shellcheck disable=SC2086
  attic push "$cache" $TO_PUSH
  echo "✅ Successfully pushed all shells to Attic $cache"
fi
