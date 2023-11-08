#!/usr/bin/env bash

version="$1"

set -eou pipefail

echo "🤛 Bumping version in Cargo.toml to ${version}"
toml set cyanprint/Cargo.toml package.version "${version}" >./cyanprint/Cargo2.toml
mv ./cyanprint/Cargo2.toml ./cyanprint/Cargo.toml
echo "🤜 Bumped version in Cargo.toml to ${version}"
