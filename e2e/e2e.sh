#!/usr/bin/env bash

set -eou pipefail

cargo build

PATH_ADDITION="$(pwd)/target/debug"
export PATH="${PATH_ADDITION}:$PATH"

cyanprint daemon

# build processors
echo "🔍 Publishing processors..."
./e2e/publish-processor.sh ./e2e/processor1 processor1
./e2e/publish-processor.sh ./e2e/processor1 processor1

./e2e/publish-processor.sh ./e2e/processor2 processor2
./e2e/publish-processor.sh ./e2e/processor2 processor2

# build plugins
echo "🔍 Publishing plugins..."
./e2e/publish-plugin.sh ./e2e/plugin1 plugin1
./e2e/publish-plugin.sh ./e2e/plugin1 plugin1

./e2e/publish-plugin.sh ./e2e/plugin2 plugin2
./e2e/publish-plugin.sh ./e2e/plugin2 plugin2

# build templates
echo "🔍 Publishing templates..."
./e2e/publish-template.sh ./e2e/template1 template1
./e2e/publish-template.sh ./e2e/template2 template2

echo "✅ Done"
