# Task Spec: Build + Push Commands

**Ticket:** CU-86ewvp51g
**Component:** Iridium (cyanprint CLI)
**Parent:** CU-86et8z88g (Local Testing Strategy)

## Summary

Implement `cyanprint build` command and enhance `cyanprint push` with `--build` option for building Docker images and publishing to the CyanPrint registry. This enables CI/CD workflows to build and publish template, processor, plugin, and resolver artifacts.

## Background

Currently, the `push` command only supports pushing pre-built Docker images. Developers must manually run `docker buildx build` commands before pushing. This task adds:

1. **`build` command** - Build Docker images using buildx CLI (pushes by default)
2. **Enhanced `push` command** - Add `--build` option to each subcommand for build+push in one step

### Design Decisions

- **Image tag format**: `{registry}/{image_name}:{tag}` (e.g., `ghcr.io/org/project/template:v1.0.0`)
- **Tag strategy**: Single `<tag>` applied to all images
- **Platform defaults**: Use `build.platforms` from config, or current platform only
- **Builder**: Use default docker builder
- **Build pushes by default**: Images are pushed to Docker registry after build
- **Authentication**: Assumes user is already logged in to Docker registry

---

## Part 1: Build Configuration

### cyan.yaml Structure (Template)

```yaml
# =============================================================================
# METADATA
# =============================================================================
username: atomi
name: nix-init
description: CyanPrint Template to initialize Nix Flake project
project: https://github.com/AtomiCloud/ketone.nix-init
source: https://github.com/AtomiCloud/ketone.nix-init.git
email: admin@atomi.cloud
tags: ['atomi']
readme: cyan/README.MD

# =============================================================================
# DEPENDENCIES (Templates only)
# =============================================================================
processors: ['cyan/default']
plugins: []
templates: []

# =============================================================================
# BUILD CONFIGURATION
# Used by: cyanprint build, cyanprint push --build
# =============================================================================
build:
  # Docker registry for images
  registry: ghcr.io/atomicloud/sulfone.ketone.nix-init

  # Supported platforms (optional)
  platforms:
    - linux/arm64
    - linux/amd64

  # Docker images to build
  images:
    template:
      dockerfile: cyan/template.Dockerfile
      context: cyan
    blob:
      dockerfile: cyan/blob.Dockerfile
      context: .
```

### cyan.yaml Structure (Processor)

```yaml
username: atomi
name: ts-transform
description: TypeScript file transformer processor
# ... metadata ...

build:
  registry: ghcr.io/atomicloud/sulfone.cyan.processors.ts-transform
  platforms:
    - linux/arm64
    - linux/amd64
  images:
    processor:
      dockerfile: Dockerfile
      context: .
```

### cyan.yaml Structure (Plugin)

```yaml
username: atomi
name: prettier-format
description: Prettier formatting plugin
# ... metadata ...

build:
  registry: ghcr.io/atomicloud/sulfone.cyan.plugins.prettier-format
  platforms:
    - linux/arm64
    - linux/amd64
  images:
    plugin:
      dockerfile: Dockerfile
      context: .
```

### cyan.yaml Structure (Resolver)

```yaml
username: atomi
name: json-merger
description: Deep merge JSON files with conflict resolution
project: atomi
source: github.com/atomi/resolvers
email: dev@atomi.com
tags: [json, merge]
readme: README.md

build:
  registry: ghcr.io/atomicloud/sulfone.cyan.resolvers.json-merger
  platforms:
    - linux/arm64
    - linux/amd64
  images:
    resolver:
      dockerfile: Dockerfile
      context: .
```

### Config Fields

| Field                 | Type     | Required    | Description                                             |
| --------------------- | -------- | ----------- | ------------------------------------------------------- |
| `registry`            | string   | Yes         | Full image reference path (e.g., `ghcr.io/org/project`) |
| `platforms`           | string[] | No          | Target platforms (default: current platform only)       |
| `images.template`     | object   | Conditional | Template image config (for templates)                   |
| `images.blob`         | object   | Conditional | Blob image config (for templates)                       |
| `images.processor`    | object   | Conditional | Processor image config (for processors)                 |
| `images.plugin`       | object   | Conditional | Plugin image config (for plugins)                       |
| `images.resolver`     | object   | Conditional | Resolver image config (for resolvers)                   |
| `images.*.dockerfile` | string   | Yes         | Path to Dockerfile                                      |
| `images.*.context`    | string   | Yes         | Build context directory                                 |

---

## Part 2: Build Command

### Usage

```bash
cyanprint build <tag> [options]
```

### Arguments

| Argument | Description                                                              |
| -------- | ------------------------------------------------------------------------ |
| `<tag>`  | Docker tag to apply to all images (e.g., `v1.0.0`, `latest`, commit SHA) |

### Options

| Option                   | Description                                                         |
| ------------------------ | ------------------------------------------------------------------- |
| `--platform <platforms>` | Target platforms (comma-separated, e.g., `linux/arm64,linux/amd64`) |
| `--builder <name>`       | Buildx builder to use                                               |
| `--no-cache`             | Don't use cache                                                     |
| `--dry-run`              | Show commands without executing                                     |

### Behavior

1. **Pre-flight checks**:

   - Verify Docker daemon is running
   - Verify buildx is available
   - Verify `cyan.yaml` exists and has `build` section
   - Verify specified builder exists (if `--builder`)

2. **Load config**: Read `cyan.yaml` and parse `build` section

3. **Validate**: Ensure `registry` is set, at least one image defined

4. **Resolve platforms**:

   - Use `--platform` option if specified
   - Else use `build.platforms` from config
   - Else use current platform only

5. **Build images**: For each image defined in config:
   - Construct full image tag: `{registry}/{image_name}:{tag}`
   - Run `docker buildx build --push` with appropriate flags
   - On `--dry-run`, print command instead of executing

### Image Tag Resolution

```
{registry}/{image_name}:{tag}

Example:
ghcr.io/atomicloud/sulfone.ketone.nix-init/template:v1.0.0
└── registry from cyan.yaml    └── image name (template/blob/processor/plugin/resolver)    └── tag from CLI
```

### buildx Command Template

```bash
docker buildx build \
  <context> \
  -f <dockerfile> \
  --platform <platforms> \
  -t <registry>/<image>:<tag> \
  --push \
  [--no-cache]
```

### Example Execution

Given `cyan.yaml`:

```yaml
build:
  registry: ghcr.io/atomicloud/sulfone.ketone.nix-init
  platforms:
    - linux/arm64
    - linux/amd64
  images:
    template:
      dockerfile: cyan/template.Dockerfile
      context: cyan
    blob:
      dockerfile: cyan/blob.Dockerfile
      context: .
```

Running:

```bash
cyanprint build v1.0.0
```

Executes:

```bash
docker buildx build \
  ./cyan \
  -f cyan/template.Dockerfile \
  --platform linux/arm64,linux/amd64 \
  -t ghcr.io/atomicloud/sulfone.ketone.nix-init/template:v1.0.0 \
  --push

docker buildx build \
  . \
  -f cyan/blob.Dockerfile \
  --platform linux/arm64,linux/amd64 \
  -t ghcr.io/atomicloud/sulfone.ketone.nix-init/blob:v1.0.0 \
  --push
```

### Output

```
Building images with tag: v1.0.0
Platforms: linux/arm64,linux/amd64

Building template image...
  Done

Building blob image...
  Done

Build complete: 2 images built and pushed
```

### Use Cases

```bash
# Build for release
cyanprint build v1.0.0

# Build without cache
cyanprint build v1.0.0 --no-cache

# CI build with custom builder
cyanprint build $COMMIT_SHA --builder ci-builder

# Dry run (debug)
cyanprint build v1.0.0 --dry-run
```

---

## Part 3: Push Command (Enhanced)

### Current Push Command Structure

The existing push command uses **subcommands** for each artifact type:

```bash
cyanprint push <artifact-type> [options] [args]
```

### Existing Subcommands

| Subcommand  | Arguments                                                 | Description                            |
| ----------- | --------------------------------------------------------- | -------------------------------------- |
| `template`  | `<blob_image> <blob_tag> <template_image> <template_tag>` | Push template with Docker images       |
| `group`     | (none)                                                    | Push template group (no Docker images) |
| `processor` | `<image> <tag>`                                           | Push processor                         |
| `plugin`    | `<image> <tag>`                                           | Push plugin                            |
| `resolver`  | `<image> <tag>`                                           | Push resolver                          |

### Enhancement: Add `--build` Option

Add `--build <tag>` option to each subcommand to enable build+push in one step:

```bash
# Build and push template
cyanprint push template --build <tag> [options]

# Build and push processor
cyanprint push processor --build <tag> [options]

# Build and push plugin
cyanprint push plugin --build <tag> [options]

# Build and push resolver
cyanprint push resolver --build <tag> [options]
```

### Updated Command Syntax

#### push template

```bash
# Existing: Push with pre-built images
cyanprint push template <blob_image> <blob_tag> <template_image> <template_tag> \
  --config cyan.yaml --token $CYAN_TOKEN --message "desc"

# NEW: Build and push
cyanprint push template --build <tag> \
  --config cyan.yaml --token $CYAN_TOKEN --message "desc"
```

#### push processor

```bash
# Existing: Push with pre-built image
cyanprint push processor <image> <tag> \
  --config cyan.yaml --token $CYAN_TOKEN --message "desc"

# NEW: Build and push
cyanprint push processor --build <tag> \
  --config cyan.yaml --token $CYAN_TOKEN --message "desc"
```

#### push plugin

```bash
# Existing: Push with pre-built image
cyanprint push plugin <image> <tag> \
  --config cyan.yaml --token $CYAN_TOKEN --message "desc"

# NEW: Build and push
cyanprint push plugin --build <tag> \
  --config cyan.yaml --token $CYAN_TOKEN --message "desc"
```

#### push resolver

```bash
# Existing: Push with pre-built image
cyanprint push resolver <image> <tag> \
  --config cyan.yaml --token $CYAN_TOKEN --message "desc"

# NEW: Build and push
cyanprint push resolver --build <tag> \
  --config cyan.yaml --token $CYAN_TOKEN --message "desc"
```

### Options (All Subcommands)

| Option      | Short | Default          | Description                                                |
| ----------- | ----- | ---------------- | ---------------------------------------------------------- |
| `--config`  | `-c`  | `cyan.yaml`      | Configuration file path                                    |
| `--token`   | `-t`  | (required)       | API token for CyanPrint registry (or `CYAN_TOKEN` env var) |
| `--message` | `-m`  | "No description" | Description for this version                               |
| `--build`   |       | (new)            | Build with tag before pushing                              |

### Build-Mode Additional Options

When `--build` is used, these options are also available:

| Option                   | Description                        |
| ------------------------ | ---------------------------------- |
| `--platform <platforms>` | Target platforms (comma-separated) |
| `--builder <name>`       | Buildx builder to use              |
| `--no-cache`             | Don't use cache                    |

### Mode Selection Logic (per subcommand)

```
For template subcommand:
  if --build provided:
    Build images using config, then register with CyanPrint registry
  elif blob_image, blob_tag, template_image, template_tag provided:
    Register with CyanPrint registry using existing images
  else:
    Error: must provide either --build or all image arguments

For processor/plugin/resolver subcommands:
  if --build provided:
    Build image using config, then register with CyanPrint registry
  elif image and tag provided:
    Register with CyanPrint registry using existing image
  else:
    Error: must provide either --build or image and tag arguments

For group subcommand:
  (no changes - groups have no Docker images)
```

### Push Flow

#### Build Mode (`--build`)

1. **Build phase** - Same as `cyanprint build <tag>`

   - Uses `docker buildx` CLI
   - Pushes to Docker registry

2. **Register phase** - Register with CyanPrint registry
   - Read `cyan.yaml` for metadata
   - Call registry API with image refs

#### Existing Mode (image arguments)

1. Read `cyan.yaml` for metadata
2. Call CyanPrint registry API to register

### Registry API Calls

Reuse existing methods in `CyanRegistryClient`:

- `push_template()` - for templates
- `push_template_without_properties()` - for groups
- `push_processor()` - for processors
- `push_plugin()` - for plugins
- `push_resolver()` - for resolvers

---

## Files to Create/Modify

### Create

| File                                          | Purpose                    |
| --------------------------------------------- | -------------------------- |
| `cyanregistry/src/cli/models/build_config.rs` | Build section config model |
| `cyanprint/src/docker/mod.rs`                 | Docker module              |
| `cyanprint/src/docker/buildx.rs`              | Buildx CLI wrapper         |

### Modify

| File                                 | Changes                                                       |
| ------------------------------------ | ------------------------------------------------------------- |
| `cyanprint/src/commands.rs`          | Add `Build` command, add `--build` option to push subcommands |
| `cyanprint/src/main.rs`              | Wire up Build command, handle `--build` in push handlers      |
| `cyanregistry/src/cli/models/mod.rs` | Export build config                                           |
| `cyanregistry/src/cli/mapper.rs`     | Add build config mapper                                       |
| `cyanregistry/src/cli/mod.rs`        | Export new models                                             |

---

## Implementation Checklist

### cyanregistry/src/cli/models/build_config.rs

- [ ] Create `BuildConfig` struct with registry, platforms, images
- [ ] Create `ImagesConfig` struct with optional template, blob, processor, plugin, resolver fields
- [ ] Create `ImageConfig` struct with dockerfile, context
- [ ] Add serde defaults for optional fields

### cyanregistry/src/cli/models/mod.rs

- [ ] Export `build_config` module

### cyanregistry/src/cli/mapper.rs

- [ ] Add `build_config_mapper()` function
- [ ] Handle default platforms (current platform only if not specified)

### cyanprint/src/docker/mod.rs

- [ ] Create docker module

### cyanprint/src/docker/buildx.rs

- [ ] Create `BuildxBuilder` struct
- [ ] Implement `build()` method that constructs and executes buildx command
- [ ] Implement `dry_run()` method for --dry-run mode
- [ ] Handle platform parsing (comma-separated string)
- [ ] Handle error output from docker CLI
- [ ] Pre-flight checks (Docker daemon, buildx availability)

### cyanprint/src/commands.rs

- [ ] Add `Build` command with tag argument and options
- [ ] Add `--build` option to `PushCommands::Template`, `Processor`, `Plugin`, `Resolver`
- [ ] Make image arguments optional when `--build` is used (mutually exclusive group)

### cyanprint/src/main.rs

- [ ] Handle `Commands::Build` - call buildx wrapper
- [ ] Update push handlers to support `--build` option
- [ ] When `--build` is used: build first, then call registry push
- [ ] Print success/error messages with emojis

---

## Error Handling

| Scenario                           | Error Message                                                   | Exit Code |
| ---------------------------------- | --------------------------------------------------------------- | --------- |
| Docker daemon not running          | `Docker daemon is not running. Please start Docker.`            | 1         |
| buildx not available               | `Docker buildx is not available. Please install buildx.`        | 1         |
| cyan.yaml not found                | `cyan.yaml not found in current directory.`                     | 1         |
| No build section                   | `No build configuration found in cyan.yaml.`                    | 1         |
| Missing `registry` field           | `Error: build.registry is required`                             | 1         |
| No images defined                  | `Error: At least one image must be defined in build.images`     | 1         |
| Builder not found                  | `Builder '{name}' not found. Run 'docker buildx create {name}'` | 1         |
| Build failed                       | Show full buildx output                                         | 1         |
| Push failed (no Docker auth)       | `Not authenticated to registry. Run 'docker login {registry}'`  | 1         |
| Not authenticated (CyanPrint)      | `Not authenticated. Set CYAN_TOKEN environment variable.`       | 1         |
| Missing image args without --build | `Error: must provide either --build or image arguments`         | 1         |

---

## Acceptance Criteria

1. **Dry run shows commands** - `--dry-run` prints buildx commands without executing
2. **Build succeeds and pushes** - `cyanprint build v1.0.0` builds and pushes all images
3. **Custom platforms** - `--platform linux/amd64` overrides config
4. **No cache option** - `--no-cache` passes flag to buildx
5. **Push template with build** - `cyanprint push template --build v1.0.0` builds and registers
6. **Push processor with build** - `cyanprint push processor --build v1.0.0` builds and registers
7. **Push plugin with build** - `cyanprint push plugin --build v1.0.0` builds and registers
8. **Push resolver with build** - `cyanprint push resolver --build v1.0.0` builds and registers
9. **Push with existing images still works** - Existing push syntax unchanged
10. **Error handling** - Clear error messages for Docker/config issues

---

## Testing Requirements

### Unit Tests

- [ ] Parse build config from YAML (template, processor, plugin, resolver variants)
- [ ] Default platforms (current platform when not specified)
- [ ] Buildx command construction (all flags)
- [ ] Image reference construction `{registry}/{image_name}:{tag}`

### Integration Tests

- [ ] `--dry-run` outputs correct commands
- [ ] Invalid config produces clear error
- [ ] Missing Docker produces clear error

### Manual E2E

- [ ] Build template images with `cyanprint build v1.0.0`
- [ ] Build processor image
- [ ] Build plugin image
- [ ] Build resolver image
- [ ] Push template with build: `cyanprint push template --build v1.0.0`
- [ ] Push processor with build: `cyanprint push processor --build v1.0.0`
- [ ] Push plugin with build: `cyanprint push plugin --build v1.0.0`
- [ ] Push resolver with build: `cyanprint push resolver --build v1.0.0`
- [ ] Verify existing push syntax still works

---

## Non-Functional Requirements

### Quality Gates

- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` passes
- [ ] `cargo fmt --check` passes

### Code Quality

- Follow existing command patterns in `commands.rs`
- Reuse existing registry client methods (`push_template`, `push_processor`, `push_plugin`, `push_resolver`)
- Keep buildx wrapper simple and focused
- Use CLI not SDK (users familiar with buildx, CI environments have custom setups)

### Documentation

- Update `docs/developer/surfaces/cli/` with new commands
- Add usage examples to README or docs

---

## Implementation Notes

### Use CLI, Not SDK

This command uses `docker buildx` CLI directly (not bollard SDK) because:

1. CI environments often have custom buildx setups
2. Buildx supports advanced caching strategies
3. Multi-platform builds require buildx
4. Users are familiar with buildx options

### Build Context

Context path is relative to `cyan.yaml` location.

### Platform Resolution Order

1. `--platform` CLI option
2. `build.platforms` from config
3. Current platform only (fallback)

### Backward Compatibility

The existing push syntax must continue to work:

```bash
cyanprint push template blob-image blob-tag template-image template-tag
cyanprint push processor image tag
cyanprint push plugin image tag
cyanprint push resolver image tag
cyanprint push group
```
