#!/usr/bin/env bash

path="$1"
template="$2"

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

# build blob
blob_image="$DOCKER_USERNAME/$template-blob"
docker buildx build \
  "." \
  -f "./blob.Dockerfile" \
  -t "$blob_image:$tag" \
  --load

# build script
script_image="$DOCKER_USERNAME/$template-script"
docker buildx build \
  "./cyan" \
  -f "./cyan/Dockerfile" \
  -t "$script_image:$tag" \
  --load

cyanprint push template "$blob_image" "$tag" "$script_image" "$tag"
