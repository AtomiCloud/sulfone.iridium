#!/usr/bin/env bash

path="$1"
resolver="$2"
build_type="${3:-load}"

[ -z "$path" ] && echo "❌ Usage: $0 <path>" && exit 1
[ -z "$resolver" ] && echo "❌ Usage: $0 <resolver>" && exit 1

[ "$CYANPRINT_USERNAME" = '' ] && echo "❌ 'CYANPRINT_USERNAME' env var not set" && exit 1
[ "$CYANPRINT_REGISTRY" = '' ] && echo "❌ 'CYANPRINT_REGISTRY' env var not set" && exit 1
[ "$CYANPRINT_COORDINATOR" = '' ] && echo "❌ 'CYANPRINT_COORDINATOR' env var not set" && exit 1
[ "$CYAN_TOKEN" = '' ] && echo "❌ 'CYAN_TOKEN' env var not set" && exit 1

[ "$DOCKER_USERNAME" = '' ] && echo "❌ 'DOCKER_USERNAME' env var not set" && exit 1

# Fix shell option syntax: order: errexit on unset, exit on error
set -euo pipefail

root="$(pwd)"
cleanup() {
  cd "$root"
}
trap cleanup EXIT
cd "$path"

tag=$(openssl rand -hex 3)

docker buildx build \
  "." \
  -f "./Dockerfile" \
  -t "$DOCKER_USERNAME/$resolver:$tag" \
  "--$build_type"

cyanprint push resolver "$DOCKER_USERNAME/$resolver" "$tag"
