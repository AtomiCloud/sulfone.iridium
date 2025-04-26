#!/usr/bin/env bash

[ "${RUST_VERSION}" = '' ] && echo "❌ 'RUST_VERSION' env var not set" && exit 1
[ "${SCOOP_BREW_TOKEN}" = '' ] && echo "❌ 'SCOOP_BREW_TOKEN' env var not set" && exit 1
[ "${FURY_TOKEN}" = '' ] && echo "❌ 'FURY_TOKEN' env var not set" && exit 1

set -eou pipefail

echo "⚙️ Generating changelog diff..."

if [ ! -f ./Changelog.md ] || [ ! -f ./Changelog.old.md ]; then
  echo "⚙️ One or both changelog files are missing. Creating an empty IncrementalChangelog.md..."
  touch ./IncrementalChangelog.md
else
  set +e
  echo "⚙️ Generating changelog diff..."
  diff --new-line-format='' --unchanged-line-format='' --old-line-format='%L' ./Changelog.md ./Changelog.old.md >./IncrementalChangelog.md
  ec="$?"
  set -e
  if [ "$ec" -ne 1 ]; then
    echo "⚠️ Changelog diff not generated"
    exit 1
  fi
fi
echo "✅ Changelog diff generated"

echo "🔨 Building release"
goreleaser release --clean --release-notes ./IncrementalChangelog.md
echo "✅ Released"

echo "🔨 Publishing to FURY"
./scripts/fury.sh
echo "✅ Published to FURY"
