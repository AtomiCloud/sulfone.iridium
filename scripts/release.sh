#!/usr/bin/env bash

set -eou pipefail

echo "🔨 Removing git hooks..."
rm -rf .git/hooks || true

echo "🔨 Running semantic-release..."
sg release -i npm || true
