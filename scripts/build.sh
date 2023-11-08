#!/usr/bin/env bash

go_arch=$1
go_os=$2
project_name=$3

set -eou pipefail

# Make Go -> Rust arch/os mapping
case "$go_arch" in
amd64) rust_arch='x86_64' ;;
arm64) rust_arch='aarch64' ;;
*) echo "unknown arch: $go_arch" && exit 1 ;;
esac
case $go_os in
linux) rust_os='linux' ;;
darwin) rust_os='apple-darwin' ;;
windows) rust_os='windows' ;;
*) echo "unknown os: $go_os" && exit 1 ;;
esac

# Detect the correct artifact directory based on existing pattern
artifact_dir=$(find artifacts -type d -name "*${rust_arch}*${rust_os}*${project_name}*")
if [[ -z $artifact_dir ]]; then
  echo "No matching artifact directory found for the specified arch/os"
  exit 1
fi

# Detect the correct dist directory based on existing structure
dist_dir=$(find dist -type d -name "${project_name}*${go_os}*${go_arch}*")
if [[ -z $dist_dir ]]; then
  echo "No matching distribution directory found for the specified arch/os"
  exit 1
fi

# Identify the binary name based on OS
binary_name="cyanprint"
if [[ $go_os == "windows" ]]; then
  binary_name="${binary_name}.exe"
fi

rm -rf "${dist_dir}"
mkdir -p "${dist_dir}"

cp "${artifact_dir}/${binary_name}" "${dist_dir}/"

echo "Copied '${binary_name}' to '${dist_dir}/'"
