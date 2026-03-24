# Plan 1: cyanregistry — Config parsing, domain models, HTTP models, mappers

## Goal

Extend the cyanregistry data pipeline to parse, carry, serialize, and deserialize preset answers on template dependency references. No runtime behavior changes — purely data model and mapping work.

## Files to change

1. `cyanregistry/src/cli/models/template_config.rs`
2. `cyanregistry/src/domain/config/template_config.rs`
3. `cyanregistry/src/cli/mapper.rs`
4. `cyanregistry/src/http/models/template_req.rs`
5. `cyanregistry/src/http/models/template_res.rs`
6. `cyanregistry/src/http/mapper.rs`

## Steps

### 1.1 Add `CyanTemplateFileRef` enum to CLI config model

**File**: `cyanregistry/src/cli/models/template_config.rs`

Add an untagged enum that accepts both plain strings and extended objects:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CyanTemplateFileRef {
    Simple(String),
    Extended {
        template: String,
        #[serde(default)]
        preset_answers: HashMap<String, serde_json::Value>,
    },
}
```

Change `CyanTemplateFileConfig.templates` from `Vec<String>` to `Vec<CyanTemplateFileRef>`.

### 1.2 Add `preset_answers` to domain model

**File**: `cyanregistry/src/domain/config/template_config.rs`

Add field to `CyanTemplateRef`:

```rust
pub struct CyanTemplateRef {
    pub username: String,
    pub name: String,
    pub version: Option<i64>,
    pub preset_answers: HashMap<String, serde_json::Value>,
}
```

### 1.3 Update CLI mapper

**File**: `cyanregistry/src/cli/mapper.rs`

- Refactor `template_reference_mapper` to accept `&CyanTemplateFileRef` instead of `String`
  - `Simple(s)`: parse string as before, set `preset_answers: HashMap::new()`
  - `Extended { template, preset_answers }`: parse the `template` string, carry `preset_answers`
- Update `template_config_mapper` to use the new mapper
- Update/add unit tests

### 1.4 Add `preset_answers` to HTTP request model

**File**: `cyanregistry/src/http/models/template_req.rs`

Add `#[serde(rename_all = "camelCase")]` to `TemplateRefReq` and add field:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateRefReq {
    pub username: String,
    pub name: String,
    pub version: i64,
    #[serde(default)]
    pub preset_answers: HashMap<String, serde_json::Value>,
}
```

### 1.5 Add `TemplateVersionTemplateRefRes` to HTTP response model

**File**: `cyanregistry/src/http/models/template_res.rs`

Add new struct (following the `TemplateVersionResolverRes` pattern):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateVersionTemplateRefRes {
    pub id: String,
    pub version: i64,
    #[serde(default)]
    pub preset_answers: HashMap<String, serde_json::Value>,
}
```

Change `TemplateVersionRes.templates` from `Vec<TemplateVersionPrincipalRes>` to `Vec<TemplateVersionTemplateRefRes>`.

**Note**: This changes a field type. All code that reads `.templates` and accesses `.id` or `.version` should still compile since those fields exist on the new type. Check all usages of `TemplateVersionRes.templates` and update any code that accesses fields not present on the new type (e.g., `.created_at`, `.description`, `.properties`).

### 1.6 Update HTTP mapper

**File**: `cyanregistry/src/http/mapper.rs`

- Update `template_ref_req_mapper` to copy `preset_answers` from `CyanTemplateRef` to `TemplateRefReq`
- Update existing `template_req_with_properties_mapper` and `template_req_without_properties_mapper` (they call `template_ref_req_mapper` via `.iter().map()`, so they should work automatically)
- Update existing test assertions for `TemplateRefReq` (add `preset_answers` field)
- Add new unit test: `test_template_ref_req_mapper_with_preset_answers`
- Add new unit test: `test_template_ref_req_mapper_without_preset_answers`

### 1.7 Fix downstream compilation

After the response model change (1.5), check and fix all code that uses `TemplateVersionRes.templates` as `Vec<TemplateVersionPrincipalRes>`:

- `cyancoordinator/src/operations/composition/resolver.rs` — accesses `.id` on template refs (still valid)
- Any other code that accesses `.properties`, `.created_at`, `.description` on template refs (will break, need fixing)

Search: `template\.templates` and `dep\.` in the codebase.

## Verification

- `cargo build` succeeds
- `cargo test` passes (existing tests updated + new tests)
- serde round-trip test: serialize/deserialize `TemplateRefReq` with preset_answers
- `pre-commit run --all` passes
