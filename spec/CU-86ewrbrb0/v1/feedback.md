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
