# Feedback for v2

## Commit Reference

c3d5f9000332c31a955a425d783e4ce976e4bddf

## Issues to Fix

### 1. `resolver_ref_config` File Structure

`CyanResolverRef` should be in `template_config.rs` alongside `CyanProcessorRef`, `CyanPluginRef`, `CyanTemplateRef` - not in its own separate file. Same for `CyanResolverRefFileConfig` in the CLI models.

### 2. `ResolverRefReq` Field Naming is Inconsistent

**Current (broken):**

```rust
pub struct ResolverRefReq {
    pub resolver_reference: String,  // "username/name" combined
    pub resolver_version: u64,        // different naming
    pub config: serde_json::Value,
    pub files: Vec<String>,
}
```

**Should match pattern of `PluginRefReq`, `ProcessorRefReq`, `TemplateRefReq`:**

```rust
pub struct ResolverRefReq {
    pub username: String,
    pub name: String,
    pub version: i64,  // consistent type with other refs
    pub config: serde_json::Value,
    pub files: Vec<String>,
}
```

The HTTP mapper (`resolver_ref_req_mapper`) also needs updating to construct `username` and `name` separately instead of combining them.

### 3. `config` Should Not Be Nullable

If there's no config, it should be `{}` (empty object), not null. Use `#[serde(default = "default_config")]`.

### 4. Missing `cyan push resolver` Command

CLI was missing the resolver push command.

### 5. `FileOrigin` Struct Doesn't Match API

**Expected API (from Helium SDK):**

```typescript
interface FileOrigin {
  readonly template: string; // just the template ID as string
  readonly layer: number; // layer index
}
```

**Current (broken):**

```rust
pub struct FileOrigin {
    pub template: TemplateInfo,  // nested struct, not a string!
}

pub struct TemplateInfo {
    pub template_id: String,
    pub template_version: i64,
    pub layer: i32,
}
```

This serializes to:

```json
{ "origin": { "template": { "template_id": "...", "template_version": 1, "layer": 0 } } }
```

But should be:

```json
{ "origin": { "template": "template-id", "layer": 0 } }
```

**Fix:** Change `FileOrigin` to:

```rust
pub struct FileOrigin {
    pub template: String,  // just template_id
    pub layer: i32,
}
```

## Serde Tags Question

**Other refs don't have serde tags on fields:**

```rust
pub struct PluginRefReq {
    pub username: String,
    pub name: String,
    #[serde(default)]  // only version has default
    pub version: i64,
}
```

**ResolverRefReq uses:**

```rust
#[serde(default = "default_config")]
pub config: serde_json::Value,
```

**Decision:** Keep `default` on config field because:

- Config is complex object, not simple number
- Matches requirement "if no config, use `{}`"
- Provides better UX than forcing explicit `config: {}`

## Questions/Clarifications (Resolved)

### Layering Implementation Structs

The following structs were reviewed:

- `ResolverInstanceInfo` - stores full resolver config for persisted state
- `TemplateResolverInfo` - lightweight runtime tracking for layering
- `TemplateVariationInfo` - fallback tracking for no-resolver case

**Decision:** These are acceptable as-is for now. Each serves a different lifecycle purpose.
