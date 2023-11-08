#!/usr/bin/env bash

version="$1"

set -eou pipefail

./scripts/bump-cargo.sh "${version}"
./scripts/bump-nix.sh "${version}"

cargo generate-lockfile
