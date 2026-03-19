#!/usr/bin/env bash

set -euo pipefail

cargo build

PATH_ADDITION="$(pwd)/target/debug"
export PATH="${PATH_ADDITION}:$PATH"

# cyanprint daemon start

# build resolvers
echo "🔍 Publishing resolvers..."
./e2e/publish-resolver.sh ./e2e/resolver1 resolver1 push

# resolver2 uses new build format
tag=$(openssl rand -hex 5)
cyanprint push --folder ./e2e/resolver2 resolver --build "$tag"

# build processors
echo "🔍 Publishing processors..."
./e2e/publish-processor.sh ./e2e/processor1 processor1 push

# processor2 uses new build format
tag=$(openssl rand -hex 5)
cyanprint push --folder ./e2e/processor2 processor --build "$tag"

# build plugins
echo "🔍 Publishing plugins..."
./e2e/publish-plugin.sh ./e2e/plugin1 plugin1 push

# plugin2 uses new build format
tag=$(openssl rand -hex 5)
cyanprint push --folder ./e2e/plugin2 plugin --build "$tag"

# build templates
echo "🔍 Publishing templates..."
./e2e/publish-template.sh ./e2e/template1 template1 push

# template2 uses new build format
tag=$(openssl rand -hex 5)
cyanprint push --folder ./e2e/template2 template --build "$tag"

./e2e/publish-template.sh ./e2e/template3 template3 push

# template5 uses new build format
tag=$(openssl rand -hex 5)
cyanprint push --folder ./e2e/template5 template --build "$tag"

./e2e/publish-template.sh ./e2e/test-batch-a-v1 test-batch-a push
./e2e/publish-template.sh ./e2e/test-batch-a-v2 test-batch-a push
./e2e/publish-template.sh ./e2e/test-batch-b-v1 test-batch-b push
./e2e/publish-template.sh ./e2e/test-batch-b-v2 test-batch-b push
./e2e/publish-template.sh ./e2e/template-resolver-1-v1 template-resolver-1 push
./e2e/publish-template.sh ./e2e/template-resolver-2-v1 template-resolver-2 push

echo "🔍 Publishing group..."
./e2e/publish-group.sh ./e2e/template4 template4 push

echo "✅ Done"
