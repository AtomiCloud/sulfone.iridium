# Plan 1: Add `image` field and `--folder` option

## Objective

Implement v2 changes:

1. Add `image` field to `ImageConfig` - separates registry from image name
2. Add `--folder` CLI option - changes working directory before running

---

## Step 1: Update BuildConfig model

**File:** `cyanregistry/src/cli/models/build_config.rs`

Add `image` field to `ImageConfig`:

```rust
pub struct ImageConfig {
    pub image: String,        // NEW: required, image name
    pub dockerfile: String,
    pub context: String,
}
```

---

## Step 2: Update build_config_mapper

**File:** `cyanregistry/src/cli/mapper.rs`

1. Validate `image` field exists and is not empty
2. Update image reference construction to use `{registry}/{image}:{tag}`

```rust
// Old: format!("{}/{}:{}", registry, image_type, tag)
// New: format!("{}/{}:{}", registry, config.image, tag)
```

---

## Step 3: Add `--folder` option to commands

**File:** `cyanprint/src/commands.rs`

Add `folder` option to:

- `Build` command
- `PushCommands::Template`, `Processor`, `Plugin`, `Resolver` (for --build mode)

```rust
#[derive(Args)]
pub struct Build {
    pub tag: String,
    #[arg(long, default_value = ".")]
    pub folder: PathBuf,
    #[arg(short, long, default_value = "cyan.yaml")]
    pub config: PathBuf,
    // ... existing options
}
```

---

## Step 4: Handle `--folder` in main.rs

**File:** `cyanprint/src/main.rs`

1. Change directory to `folder` before loading config
2. Resolve `config` path relative to `folder`

```rust
// Pseudo-code
let folder = args.folder.canonicalize()?;
std::env::set_current_dir(&folder)?;
let config_path = &args.config; // Already relative to folder
```

---

## Step 5: Update buildx command construction

**File:** `cyanprint/src/docker/buildx.rs`

Update image tag format:

```rust
// Old: format!("{}/{}:{}", registry, image_type, tag)
// New: format!("{}/{}:{}", registry, image_config.image, tag)
```

---

## Step 6: Update e2e fixtures

**Files:** `e2e/plugin2/cyan.yaml`, `e2e/processor2/cyan.yaml`, `e2e/resolver2/cyan.yaml`, `e2e/template2/cyan.yaml`

Add `image` field to each:

```yaml
images:
  plugin:
    image: plugin2
    dockerfile: Dockerfile
    context: .
```

---

## Acceptance Criteria

- [ ] `image` field is required in config
- [ ] Missing `image` field produces clear error
- [ ] `--folder` option changes working directory
- [ ] `--config` is resolved relative to `--folder`
- [ ] Image tags are `{registry}/{image}:{tag}`
- [ ] Existing tests pass
- [ ] e2e fixtures updated

---

## Testing

1. Unit tests for config parsing with `image` field
2. Unit tests for missing `image` field error
3. Manual test: `cyanprint build v1 --folder ./e2e/plugin2 --config cyan.yaml --dry-run`
