# Plan 2: Push Command Enhancement with --build

**Ticket:** CU-86ewvp51g
**Goal:** Add `--build` option to push subcommands for build+push in one step

## Scope

This plan covers:

1. Add `--build` option to push subcommands
2. Make image arguments optional when --build is used
3. Add build-mode options (--platform, --builder, --no-cache, --dry-run)
4. Update push handlers to support build mode
5. Update documentation

**Depends on:** Plan 1 (uses `BuildConfig`, `BuildxBuilder`, and config parsing)

## Files to Modify

| File                                     | Changes                                                          |
| ---------------------------------------- | ---------------------------------------------------------------- |
| `cyanprint/src/commands.rs`              | Add --build option to push subcommands, make image args optional |
| `cyanprint/src/main.rs`                  | Update push handlers for --build mode                            |
| `docs/developer/surfaces/cli/01-push.md` | Document --build option                                          |

## Implementation Approach

### 1. Update PushCommands Structure

Modify `cyanprint/src/commands.rs` to add `--build` option to each subcommand.

Use optional args with validation in handler:

```rust
#[derive(Debug, Subcommand)]
pub enum PushCommands {
    Template {
        #[arg(long, help = "Build with tag before pushing")]
        build: Option<String>,

        blob_image: Option<String>,
        blob_tag: Option<String>,
        template_image: Option<String>,
        template_tag: Option<String>,
    },
    #[command(about = "Push a template group")]
    Group,
    Processor {
        #[arg(long, help = "Build with tag before pushing")]
        build: Option<String>,

        image: Option<String>,
        tag: Option<String>,
    },
    Plugin {
        #[arg(long, help = "Build with tag before pushing")]
        build: Option<String>,

        image: Option<String>,
        tag: Option<String>,
    },
    Resolver {
        #[arg(long, help = "Build with tag before pushing")]
        build: Option<String>,

        image: Option<String>,
        tag: Option<String>,
    },
}
```

Then validate in handler: either --build XOR all image args required.

### 2. Add Build-Mode Options to PushArgs

Add to `PushArgs`:

```rust
#[arg(long, help = "Target platforms (comma-separated)")]
pub platform: Option<String>,

#[arg(long, help = "Buildx builder to use")]
pub builder: Option<String>,

#[arg(long, help = "Don't use cache")]
pub no_cache: bool,

#[arg(long, help = "Show commands without executing")]
pub dry_run: bool,
```

### 3. Update Push Handlers

For each push handler in `main.rs`:

**Template handler pattern:**

```rust
PushCommands::Template { build, blob_image, blob_tag, template_image, template_tag } => {
    let PushArgs { config, token, message, platform, builder, no_cache, dry_run } = push_arg;

    let (blob_ref, blob_tag_val, template_ref, template_tag_val) = if let Some(tag) = build {
        // Build mode
        // 1. Load config and validate build section exists
        // 2. Run build using BuildxBuilder (from Plan 1)
        // 3. Construct refs from config: {registry}/blob:{tag}, {registry}/template:{tag}
        // 4. Return refs
    } else {
        // Push existing mode
        match (blob_image, blob_tag, template_image, template_tag) {
            (Some(bi), Some(bt), Some(ti), Some(tt)) => (bi, bt, ti, tt),
            _ => {
                eprintln!("Error: must provide either --build or all image arguments");
                return Err(...);
            }
        }
    };

    // Call registry.push_template() with refs
    let res = registry.push_template(config, token, message, blob_ref, blob_tag_val, template_ref, template_tag_val);
    // ... handle result
}
```

**Processor/Plugin/Resolver handler pattern:**

```rust
PushCommands::Processor { build, image, tag } => {
    let PushArgs { config, token, message, platform, builder, no_cache, dry_run } = push_arg;

    let (image_ref, tag_val) = if let Some(tag) = build {
        // Build mode
        // 1. Load config and validate build section exists
        // 2. Run build using BuildxBuilder for processor image
        // 3. Construct ref from config: {registry}/processor:{tag}
        // 4. Return ref
    } else {
        // Push existing mode
        match (image, tag) {
            (Some(i), Some(t)) => (i, t),
            _ => {
                eprintln!("Error: must provide either --build or image and tag");
                return Err(...);
            }
        }
    };

    // Call registry.push_processor() with ref
    let res = registry.push_processor(config, token, message, image_ref, tag_val);
    // ... handle result
}
```

### 4. Reuse Build Logic from Plan 1

Create a shared build function that can be called from both:

- `Commands::Build` handler
- Push handlers when --build is used

```rust
// In cyanprint/src/build.rs or similar
pub fn run_build(
    config_path: &str,
    tag: &str,
    platform: Option<&str>,
    builder: Option<&str>,
    no_cache: bool,
    dry_run: bool,
) -> Result<BuildResult, Box<dyn Error + Send>>

pub struct BuildResult {
    pub registry: String,
    pub images_built: Vec<String>, // ["template", "blob"] or ["processor"], etc.
}
```

### 5. Update Documentation

Update `docs/developer/surfaces/cli/01-push.md`:

- Add --build option to Options table
- Add Build-Mode Options section
- Add examples for --build usage
- Update subcommand details with --build examples

## Edge Cases

| Scenario                        | Handling                                                   |
| ------------------------------- | ---------------------------------------------------------- |
| --build with no build section   | Error: "No build configuration found in cyan.yaml"         |
| --build with partial image args | Error: "--build cannot be used with image arguments"       |
| Neither --build nor image args  | Error: "must provide either --build or image arguments"    |
| --build on group subcommand     | Error: "group does not support --build (no Docker images)" |
| Build fails during push         | Show error, don't call registry API                        |

## Backward Compatibility

The existing push syntax must continue to work:

```bash
cyanprint push template blob-img blob-tag tmpl-img tmpl-tag
cyanprint push processor img tag
cyanprint push plugin img tag
cyanprint push resolver img tag
cyanprint push group
```

## TODO: Manual Testing

- [ ] `cyanprint push template --build v1.0.0` builds and registers
- [ ] `cyanprint push processor --build v1.0.0` builds and registers
- [ ] `cyanprint push plugin --build v1.0.0` builds and registers
- [ ] `cyanprint push resolver --build v1.0.0` builds and registers
- [ ] Existing push syntax still works
- [ ] `cyanprint push group` still works (no --build option)

## Integration with Plan 1

Plan 1 provides:

- `BuildConfig` struct - used to parse build section
- `BuildxBuilder` - used to execute docker buildx
- Config loading logic - reused

This plan:

- Uses BuildxBuilder in push handlers
- Calls config parsing to get registry/image names
- Constructs image refs from build result

## Implementation Checklist

- [ ] Update `cyanprint/src/commands.rs`

  - [ ] Add `--build` option to Template subcommand
  - [ ] Add `--build` option to Processor subcommand
  - [ ] Add `--build` option to Plugin subcommand
  - [ ] Add `--build` option to Resolver subcommand
  - [ ] Make image arguments optional (Option<String>)
  - [ ] Add `--platform`, `--builder`, `--no-cache`, `--dry-run` to PushArgs

- [ ] Update `cyanprint/src/main.rs`

  - [ ] Extract build-mode options from PushArgs
  - [ ] Template handler: support --build mode
  - [ ] Processor handler: support --build mode
  - [ ] Plugin handler: support --build mode
  - [ ] Resolver handler: support --build mode
  - [ ] Validate: --build XOR image args
  - [ ] Group handler: reject --build option
  - [ ] Reuse BuildxBuilder from Plan 1
  - [ ] Construct image refs from config when --build

- [ ] Create shared build helper (optional but recommended)

  - [ ] Factor out build logic for reuse
  - [ ] Return BuildResult with registry and images built

- [ ] Update `docs/developer/surfaces/cli/01-push.md`
  - [ ] Add --build to Options table
  - [ ] Add Build-Mode Options section
  - [ ] Add --build examples for each subcommand
  - [ ] Update subcommand details
