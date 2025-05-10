#!/usr/bin/env bash

[ "${FURY_TOKEN}" = '' ] && echo "❌ 'FURY_TOKEN' env var not set" && exit 1

set -eou pipefail

directory="dist"

echo "⬆️ Pushing APT packages fury.io"
# Use the find command to locate all .deb files in the directory
find "$directory" -type f -name "*.deb" | while read -r file; do
  echo "⏫ pushing $file to fury.io"
  curl -F package=@"$file" "https://${FURY_TOKEN}@push.fury.io/atomicloud/"
  echo "✅ pushed $file to fury.io"
done

echo "⬆️ Pushing YUM packages fury.io"
find "$directory" -type f -name "*.rpm" | while read -r file; do
  echo "⏫ pushing $file to fury.io"
  curl -F package=@"$file" "https://${FURY_TOKEN}@push.fury.io/atomicloud/"
  echo "✅ pushed $file to fury.io"
done

echo "⬆️ Pushing APK packages fury.io"
find "$directory" -type f -name "*.apk" | while read -r file; do
  echo "⏫ pushing $file to fury.io"
  curl -F package=@"$file" "https://${FURY_TOKEN}@push.fury.io/atomicloud/"
  echo "✅ pushed $file to fury.io"
done
