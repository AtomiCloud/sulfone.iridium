# Plan 1: Build Command and Config Parsing

**Ticket:** CU-86ewvp51g
**Goal:** Implement `cyanprint build <tag>` command with buildx CLI integration and config parsing

## Scope

This plan covers:

1. Build configuration model in cyanregistry
2. Buildx CLI wrapper in cyanprint
3. Build command definition and handler
4. Pre-flight checks and error handling
5. Documentation

## Files to Create

| File                                          | Purpose                     |
| --------------------------------------------- | --------------------------- |
| `cyanregistry/src/cli/models/build_config.rs` | Build section config model  |
| `cyanprint/src/docker/mod.rs`                 | Docker module               |
| `cyanprint/src/docker/buildx.rs`              | Buildx CLI wrapper          |
| `docs/developer/surfaces/cli/05-build.md`     | Build command documentation |

## Files to Modify

| File                                 | Changes                              |
| ------------------------------------ | ------------------------------------ |
| `cyanprint/src/commands.rs`          | Add `Build` command struct           |
| `cyanprint/src/main.rs`              | Add `Commands::Build` handler        |
| `cyanregistry/src/cli/models/mod.rs` | Export `build_config` module         |
| `cyanregistry/src/cli/mapper.rs`     | Add `build_config_mapper()` function |
| `cyanregistry/src/cli/mod.rs`        | Export new models                    |

## Implementation Approach

### 1. Build Config Model

Create `cyanregistry/src/cli/models/build_config.rs`:

```rust
// Structures for parsing build section from cyan.yaml
pub struct BuildConfig {
    pub registry: String,
    pub platforms: Option<Vec<String>>,
    pub images: ImagesConfig,
}

pub struct ImagesConfig {
    pub template: Option<ImageConfig>,
    pub blob: Option<ImageConfig>,
    pub processor: Option<ImageConfig>,
    pub plugin: Option<ImageConfig>,
    pub resolver: Option<ImageConfig>,
}

pub struct ImageConfig {
    pub dockerfile: String,
    pub context: String,
}
```

Use serde with `#[serde(default)]` for optional fields.

### 2. Build Config Mapper

Add to `cyanregistry/src/cli/mapper.rs`:

- Return domain `BuildConfig` struct
- Handle missing fields with clear errors

### 3. Buildx CLI Wrapper

Create `cyanprint/src/docker/buildx.rs`:

- `BuildxBuilder` struct
- `check_docker()` - verify Docker daemon running
- `check_buildx()` - verify buildx available
- `build()` method that constructs and executes buildx command
- `dry_run()` method for --dry-run mode
- Handle platform parsing (comma-separated string to array)
- Capture and report buildx output/errors

Key method signature:

```rust
pub fn build(
    &self,
    registry: &str,
    image_name: &str,
    tag: &str,
    dockerfile: &str,
    context: &str,
    platforms: &[String],
    no_cache: bool,
    dry_run: bool,
) -> Result<(), Box<dyn Error + Send>>
```

### 4. Build Command Definition

Add to `cyanprint/src/commands.rs`:

```rust
#[command(alias = "b", about = "Build Docker images using buildx")]
Build {
    #[arg(value_name = "TAG")]
    tag: String,

    #[arg(short, long, default_value = "cyan.yaml")]
    config: String,

    #[arg(short, long, help = "Target platforms (comma-separated)")]
    platform: Option<String>,

    #[arg(short, long, help = "Buildx builder to use")]
    builder: Option<String>,

    #[arg(long, help = "Don't use cache")]
    no_cache: bool,

    #[arg(long, help = "Show commands without executing")]
    dry_run: bool,
},
```

### 5. Build Command Handler

In `cyanprint/src/main.rs`, add handler for `Commands::Build`:

1. Run pre-flight checks (Docker, buildx)
2. Load and parse config file
3. Validate build section exists
4. Resolve platforms (CLI → config → current platform)
5. For each image defined in config:
   - Construct image tag: `{registry}/{image_name}:{tag}`
   - Call buildx wrapper
   - Report progress with emojis
6. Print summary

### 6. Documentation

Create `docs/developer/surfaces/cli/05-build.md` following the convention in `01-push.md`:

- Usage section with command syntax
- Description
- Options table
- Example executions
- Exit codes table
- Related commands

## Edge Cases

| Scenario                  | Handling                                                        |
| ------------------------- | --------------------------------------------------------------- |
| No build section          | Error: "No build configuration found in cyan.yaml"              |
| Missing registry          | Error: "build.registry is required"                             |
| No images defined         | Error: "At least one image must be defined in build.images"     |
| Docker not running        | Error: "Docker daemon is not running. Please start Docker."     |
| buildx not available      | Error: "Docker buildx is not available. Please install buildx." |
| Build failure             | Show full buildx output, exit with error                        |
| Empty platforms in config | Use current platform only                                       |

## Testing Strategy

### Unit Tests

- Parse build config from YAML (all artifact types)
- Platform resolution logic
- Image reference construction
- Buildx command construction

### TODO: Manual Testing

- `cyanprint build v1.0.0 --dry-run` shows commands
- `cyanprint build v1.0.0` builds and pushes images
- `cyanprint build v1.0.0 --platform linux/amd64` single platform
- `cyanprint build v1.0.0 --no-cache` without cache

## Integration with Plan 2

This plan provides:

- `BuildConfig` struct used by push --build
- `BuildxBuilder` wrapper used by push --build
- Config parsing logic shared with push

Plan 2 will:

- Use the buildx wrapper to implement --build in push
- Reuse the build config parsing
- Add --build flag handling to push subcommands

## Implementation Checklist

- [ ] Create `cyanregistry/src/cli/models/build_config.rs`

  - [ ] `BuildConfig` struct with registry, platforms, images
  - [ ] `ImagesConfig` struct with optional template, blob, processor, plugin, resolver
  - [ ] `ImageConfig` struct with dockerfile, context
  - [ ] Serde defaults for optional fields

- [ ] Update `cyanregistry/src/cli/models/mod.rs`

  - [ ] Export `build_config` module

- [ ] Update `cyanregistry/src/cli/mapper.rs`

  - [ ] Add `build_config_mapper()` function
  - [ ] Handle missing fields with clear errors

- [ ] Update `cyanregistry/src/cli/mod.rs`

  - [ ] Export new build config types

- [ ] Create `cyanprint/src/docker/mod.rs`

  - [ ] Create docker module

- [ ] Create `cyanprint/src/docker/buildx.rs`

  - [ ] `BuildxBuilder` struct
  - [ ] `check_docker()` - verify Docker daemon
  - [ ] `check_buildx()` - verify buildx available
  - [ ] `build()` - execute buildx command
  - [ ] `dry_run()` - print command without executing
  - [ ] Platform parsing (comma-separated)
  - [ ] Error output handling

- [ ] Update `cyanprint/src/commands.rs`

  - [ ] Add `Build` command variant
  - [ ] Add tag argument
  - [ ] Add --config, --platform, --builder, --no-cache, --dry-run options

- [ ] Update `cyanprint/src/main.rs`

  - [ ] Handle `Commands::Build`
  - [ ] Pre-flight checks
  - [ ] Config loading and validation
  - [ ] Platform resolution
  - [ ] Build loop for all images
  - [ ] Progress output with emojis

- [ ] Create `docs/developer/surfaces/cli/05-build.md`
  - [ ] Usage section
  - [ ] Description
  - [ ] Options table
  - [ ] Example executions
  - [ ] Exit codes
  - [ ] Related commands
