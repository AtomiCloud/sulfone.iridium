#!/usr/bin/env bash

version="$1"

set -eou pipefail

new_line="  version = \"${version}\"; # replace"

echo "🤛 Bumping version in nix to ${version}"
sed -i "/# replace/c\\$new_line" nix/default.nix
echo "🤜 Bumped version in nix to ${version}"
