#!/usr/bin/env bash

set -eou pipefail

cargo clean

pre-commit run --all
