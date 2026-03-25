# Plan 1: Data Pipeline — Parse, Push, Fetch command configs

## Goal

Extend the cyanregistry data pipeline to parse `commands` from cyan.yaml, include them in push requests to Zinc, and handle them in response models — fully backward compatible.

## Scope

- cyanregistry crate only
- No execution logic (plan 2)
- No Zinc backend changes (sister ticket)

## Files to Modify

### 1. cyanregistry/src/cli/models/template_config.rs

Add `commands` field to `CyanTemplateFileConfig`:

```rust
#[serde(default)]
pub commands: Vec<String>,
```

Place after `resolvers` field. Use `#[serde(default)]` for backward compatibility with old cyan.yaml files.

### 2. cyanregistry/src/domain/config/template_config.rs

Add `commands: Vec<String>` to `CyanTemplateConfig`.

Follow the existing pattern: add the field alongside `resolvers`.

### 3. cyanregistry/src/cli/mapper.rs

In the template config mapper function, copy `commands` from `CyanTemplateFileConfig` to `CyanTemplateConfig`.

Look for where `resolvers` is mapped and add `commands` mapping in the same spot.

### 4. cyanregistry/src/http/models/template_req.rs

Add to `TemplateReq`:

```rust
#[serde(default)]
pub commands: Vec<String>,
```

Place after `resolvers`. No `rename_all` change needed since `commands` is the same in both Rust and JSON.

### 5. cyanregistry/src/http/models/template_res.rs

Add to `TemplateVersionRes`:

```rust
#[serde(default)]
pub commands: Vec<String>,
```

This handles backward compatibility with older Zinc APIs that don't return `commands`.

### 6. cyanregistry/src/http/mapper.rs

In the template request mapper, copy `commands` from `CyanTemplateConfig` to `TemplateReq`.

**Filter empty/whitespace commands before push**: Zinc validates that each command string is non-empty and non-whitespace. Strip any empty or whitespace-only strings from the commands vec during mapping to prevent 400 errors from Zinc.

```rust
commands: config.commands.iter().filter(|c| !c.trim().is_empty()).cloned().collect()
```

Add a unit test that verifies `commands` round-trips through the mapper (empty vec, single command, multiple commands, empty strings filtered out).

## Testing Strategy

- Unit test: parse cyan.yaml with `commands` field → verify populated
- Unit test: parse cyan.yaml without `commands` field → verify empty vec (backward compat)
- Unit test: CLI mapper with commands
- Unit test: HTTP request mapper with commands (empty, populated, empty strings filtered)
- Unit test: serde round-trip for `TemplateReq` with and without `commands`
- Unit test: serde round-trip for `TemplateVersionRes` with and without `commands`
- Verify existing tests still pass (no regressions from added fields with defaults)

## Implementation Checklist

- [ ] Add `commands: Vec<String>` to `CyanTemplateFileConfig` with `#[serde(default)]`
- [ ] Add `commands: Vec<String>` to `CyanTemplateConfig`
- [ ] Update CLI mapper to map `commands`
- [ ] Add `commands: Vec<String>` to `TemplateReq` with `#[serde(default)]`
- [ ] Add `commands: Vec<String>` to `TemplateVersionRes` with `#[serde(default)]`
- [ ] Update HTTP mapper to map `commands` with empty/whitespace filtering
- [ ] Add unit tests for all changes
- [ ] Verify existing tests pass

## Integration with Plan 2

Plan 2 reads `commands` from `TemplateVersionRes` (added here) and collects them during composition. Plan 2 depends on changes 5 and 6 from this plan.
