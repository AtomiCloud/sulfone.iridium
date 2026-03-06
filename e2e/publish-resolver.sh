#!/usr/bin/env bash

path="$1"
resolver="$2"
build_type="${3:-load}"

[ "$CYANPRINT_USERNAME" = '' ] && echo "❌ 'CYANPRINT_USERNAME' env var not set" && exit 1
[ "$CYANPRINT_REGISTRY" = '' ] && echo "❌ 'CYANPRINT_REGISTRY' env var not set" && exit 1
[ "$CYANPRINT_COORDINATOR" = '' ] && echo "❌ 'CYANPRINT_COORDINATOR' env var not set" && exit 1
[ "$CYAN_TOKEN" = '' ] && echo "❌ 'CYAN_TOKEN' env var not set" && exit 1

[ "$DOCKER_USERNAME" = '' ] && echo "❌ 'DOCKER_USERNAME' env var not set" && exit 1
set -eou pipefail

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
