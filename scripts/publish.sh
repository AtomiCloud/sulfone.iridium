#!/usr/bin/env bash

set -eou pipefail

goreleaser release --clean --skip=validate --release-notes Changelog.md

./scripts/fury.sh
