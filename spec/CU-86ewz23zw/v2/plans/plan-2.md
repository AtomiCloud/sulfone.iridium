# Plan 2: Create template6 — nested template generator

## Spec requirement

R5: Create template6 — nested template generator

## Overview

Create `e2e/template6`, a cyanprint template whose output is another valid cyanprint template (template7). This tests nested/recursive templating under high parallelism.

## Steps

### 1. Create `e2e/template6/cyan.yaml`

Template metadata following the standard pattern. Key fields:

- `username: cyane2e`, `name: template6`
- `processors: ['cyane2e/processor2']`, `plugins: ['cyane2e/plugin2']`
- `build.registry: kirinnee`, `platforms: linux/amd64`
- Two images: `template` (from cyan/Dockerfile, context ./cyan) and `blob` (from blob.Dockerfile, context .)

### 2. Create `e2e/template6/cyan/index.ts`

Template service that asks 2 Text questions:

1. `templateName` — name for the generated template
2. `authorName` — author name for the generated cyan.yaml

Uses `i.text()` for both questions. Returns processor2 config with variables for templateName and authorName.

### 3. Create `e2e/template6/cyan/package.json`

Standard bun project config with `@atomicloud/cyan-sdk` dependency. Same pattern as template2.

### 4. Create `e2e/template6/cyan/tsconfig.json`

Standard TypeScript config for bun.

### 5. Create `e2e/template6/cyan/Dockerfile`

Same pattern as template2: oven/bun base, copy deps, bun install, copy source, run index.ts.

### 6. Create `e2e/template6/cyan/README.MD`

Simple readme for the template.

### 7. Create `e2e/template6/blob.Dockerfile`

Same multi-stage pattern as template2: alpine base, tar.gz creation and extraction.

### 8. Create `e2e/template6/template/` directory

The template output files (what template6 generates as template7):

- `cyan.yaml` — template metadata with `{{templateName}}` and `{{authorName}}` variables
- `cyan/Dockerfile` — simple template service Dockerfile for the generated template
- `cyan/index.ts` — simple template service with one Text question ("project name")
- `cyan/package.json` — bun project config
- `cyan/template/health.yaml` — health endpoint definition
- `blob.Dockerfile` — blob builder

All files should use processor2 variable substitution: `{{templateName}}` and `{{authorName}}`.

### 9. Create `e2e/template6/test.cyan.yaml`

5 test cases with snapshot comparison:

| #   | name     | author | test name     |
| --- | -------- | ------ | ------------- |
| 1   | my-lib   | alice  | my-lib:alice  |
| 2   | my-app   | bob    | my-app:bob    |
| 3   | api-svc  | carol  | api-svc:carol |
| 4   | cli-tool | dave   | cli-tool:dave |
| 5   | web-app  | eve    | web-app:eve   |

Format follows the spec's test.cyan.yaml format with `answer_state` using String type and `expected.type: snapshot`.

### 10. Create `e2e/template6/fixtures/expected/` directories

5 empty directories for initial snapshots (to be populated by `--update-snapshots`):

- `my-lib:alice/`, `my-app:bob/`, `api-svc:carol/`, `cli-tool:dave/`, `web-app:eve/`

### 11. Add template6 push to `e2e/build.sh`

Add after existing template pushes:

```bash
echo "🔍 Publishing template6..."
tag=$(openssl rand -hex 5)
cyanprint push --folder ./e2e/template6 template --build "$tag"
```

## Files

| Action | File                                                                                                    |
| ------ | ------------------------------------------------------------------------------------------------------- |
| Create | `e2e/template6/cyan.yaml`                                                                               |
| Create | `e2e/template6/cyan/Dockerfile`                                                                         |
| Create | `e2e/template6/cyan/index.ts`                                                                           |
| Create | `e2e/template6/cyan/package.json`                                                                       |
| Create | `e2e/template6/cyan/tsconfig.json`                                                                      |
| Create | `e2e/template6/cyan/README.MD`                                                                          |
| Create | `e2e/template6/blob.Dockerfile`                                                                         |
| Create | `e2e/template6/template/` (cyan.yaml, Dockerfile, index.ts, package.json, health.yaml, blob.Dockerfile) |
| Create | `e2e/template6/test.cyan.yaml`                                                                          |
| Create | `e2e/template6/fixtures/expected/` (5 dirs)                                                             |
| Modify | `e2e/build.sh` (add template6 push)                                                                     |

## No Rust code changes
