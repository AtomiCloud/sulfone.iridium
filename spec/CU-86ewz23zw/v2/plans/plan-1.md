# Plan 1: Split e2e.sh into build/local/full phases

## Spec requirement

R1: Split e2e.sh into 3 independent phases

## Overview

Rewrite `e2e/e2e.sh` as a dispatcher that delegates to three sub-scripts. Extract the current build/push logic into `e2e/build.sh`. Create skeleton `e2e/local.sh` and `e2e/full.sh` that will be populated in later plans.

## Steps

### 1. Create `e2e/build.sh`

Extract the current `e2e/e2e.sh` build/push logic into `e2e/build.sh`. This includes:

- `set -euo pipefail`
- `cargo build` + PATH setup
- Publish resolvers (resolver1, resolver2)
- Publish processors (processor1, processor2)
- Publish plugins (plugin1, plugin2)
- Publish templates (template1-3, template5, test-batch-a/b v1/v2, template-resolver-1/2)
- Publish template4 group
- No changes to the actual commands — just move them verbatim

### 2. Create `e2e/local.sh` (skeleton)

Create `e2e/local.sh` with:

- `set -euo pipefail`
- `cargo build` + PATH setup (needed for cyanprint binary)
- `echo "Running local tests..."`
- Placeholder comments for test commands, try commands, test init, and stress tests (to be filled in plans 2-3)
- `echo "✅ Local tests passed"`

### 3. Create `e2e/full.sh` (skeleton)

Create `e2e/full.sh` with:

- `set -euo pipefail`
- `cargo build` + PATH setup
- `echo "Running full-cycle tests..."`
- Placeholder comments for create, upgrade, conflict, and resolver commands (to be filled in plan 4)
- `echo "✅ Full-cycle tests passed"`

### 4. Rewrite `e2e/e2e.sh` as dispatcher

Replace current content with argument parsing that delegates to sub-scripts:

- No default — must specify `build`, `local`, or `full`
- Validate argument, print usage on invalid/missing
- `exec` the appropriate sub-script

## Files

| Action | File                      |
| ------ | ------------------------- |
| Create | `e2e/build.sh`            |
| Create | `e2e/local.sh` (skeleton) |
| Create | `e2e/full.sh` (skeleton)  |
| Modify | `e2e/e2e.sh`              |

## No Rust code changes
