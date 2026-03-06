#!/usr/bin/env bash

set -eou pipefail

cargo build

PATH_ADDITION="$(pwd)/target/debug"
export PATH="${PATH_ADDITION}:$PATH"

cyanprint daemon

# build resolvers
echo "🔍 Publishing resolvers..."
./e2e/publish-resolver.sh ./e2e/resolver1 resolver1 push
./e2e/publish-resolver.sh ./e2e/resolver1 resolver1 push

./e2e/publish-resolver.sh ./e2e/resolver2 resolver2 push
./e2e/publish-resolver.sh ./e2e/resolver2 resolver2 push

# build processors
echo "🔍 Publishing processors..."
./e2e/publish-processor.sh ./e2e/processor1 processor1 push
./e2e/publish-processor.sh ./e2e/processor1 processor1 push

./e2e/publish-processor.sh ./e2e/processor2 processor2 push
./e2e/publish-processor.sh ./e2e/processor2 processor2 push

# build plugins
echo "🔍 Publishing plugins..."
./e2e/publish-plugin.sh ./e2e/plugin1 plugin1 push
./e2e/publish-plugin.sh ./e2e/plugin1 plugin1 push

./e2e/publish-plugin.sh ./e2e/plugin2 plugin2 push
./e2e/publish-plugin.sh ./e2e/plugin2 plugin2 push

# build templates
echo "🔍 Publishing templates..."
./e2e/publish-template.sh ./e2e/template1 template1 push
./e2e/publish-template.sh ./e2e/template2 template2 push
./e2e/publish-template.sh ./e2e/template3 template3 push
./e2e/publish-template.sh ./e2e/test-batch-a-v1 test-batch-a push
./e2e/publish-template.sh ./e2e/test-batch-a-v2 test-batch-a push
./e2e/publish-template.sh ./e2e/test-batch-b-v1 test-batch-b push
./e2e/publish-template.sh ./e2e/test-batch-b-v2 test-batch-b push
./e2e/publish-template.sh ./e2e/test-conflict-a test-conflict-a push
./e2e/publish-template.sh ./e2e/test-conflict-b test-conflict-b push

echo "🔍 Publishing group..."
./e2e/publish-group.sh ./e2e/template4 template4 push

echo "✅ Done"
