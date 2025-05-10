#!/usr/bin/env bash

set -eou pipefail

if [ -f "Changelog.md" ]; then
  echo "⬅️ Previous changelog file found, copying to Changelog.old.md..."
  cp Changelog.md Changelog.old.md
  echo "✅ Previous changelog file copied."
else
  echo "⚠️ Previous changelog file not found, skipping..."
fi
