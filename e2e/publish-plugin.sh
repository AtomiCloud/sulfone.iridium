#!/usr/bin/env bash

path="$1"
plugin="$2"

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
  -t "$DOCKER_USERNAME/$plugin:$tag" \
  --load

cyanprint push plugin "$DOCKER_USERNAME/$plugin" "$tag"
