#!/usr/bin/env bash

set -eou pipefail

echo "🔍 Running pre-commit hooks..."
pre-commit run --all

echo "✅ Done!"
