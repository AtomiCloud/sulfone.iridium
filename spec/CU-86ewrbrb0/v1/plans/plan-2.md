# Plan 2: Template Push with Resolvers (cyanregistry)

## Goal

Extend template push to include resolvers from `cyan.yaml`. When pushing a template, also push its declared resolvers.

## Scope

- Update `CyanTemplateFileConfig` to include `resolvers` field
- Update `push_template()` to also push resolvers
- Follow same pattern as plugins/processors

## cyan.yaml Structure

```yaml
name: my-template
version: 1.0.0
# ... other fields ...
resolvers:
  - resolver: 'atomi/json-merger:1'
    config:
      strategy: 'deep-merge'
    files:
      - 'package.json'
      - '**/tsconfig.json'
  - resolver: 'atomi/line-merger:1'
    config:
      strategy: 'append'
    files:
      - '.gitignore'
```

## Files to Modify

| File                                                | Changes                                      |
| --------------------------------------------------- | -------------------------------------------- |
| `cyanregistry/src/cli/models/template_config.rs`    | Add `resolvers` field                        |
| `cyanregistry/src/domain/config/template_config.rs` | Add `resolvers` field                        |
| `cyanregistry/src/cli/mapper.rs`                    | Map resolvers from YAML                      |
| `cyanregistry/src/http/mapper.rs`                   | Map resolvers to request                     |
| `cyanregistry/src/http/models/template_req.rs`      | Add `resolvers` to request                   |
| `cyanregistry/src/http/client.rs`                   | Update `push_template()` to handle resolvers |

## Implementation Order

### Step 1: Update Template Config Models

File: `cyanregistry/src/cli/models/template_config.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CyanTemplateFileConfig {
    // ... existing fields ...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolvers: Option<Vec<CyanResolverFileConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CyanResolverFileConfig {
    pub resolver: String,  // "username/name:version"
    pub config: serde_json::Value,
    pub files: Vec<String>,  // Glob patterns
}
```

File: `cyanregistry/src/domain/config/template_config.rs`

```rust
#[derive(Debug, Clone)]
pub struct TemplateConfig {
    // ... existing fields ...
    pub resolvers: Option<Vec<ResolverConfig>>,
}

#[derive(Debug, Clone)]
pub struct ResolverConfig {
    pub resolver_ref: String,
    pub config: serde_json::Value,
    pub file_patterns: Vec<String>,
}
```

### Step 2: Update Mappers

File: `cyanregistry/src/cli/mapper.rs`

- [ ] Add `resolver_config_mapper()` function
- [ ] Update `template_config_mapper()` to include resolvers

File: `cyanregistry/src/http/mapper.rs`

- [ ] Add function to map `ResolverConfig` to `ResolverReferenceReq`
- [ ] Update `template_req_with_properties_mapper()` to include resolvers

### Step 3: Update Template Request Models

File: `cyanregistry/src/http/models/template_req.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePropertyReq {
    // ... existing fields ...
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverReferenceReq {
    pub resolver: String,
    pub config: serde_json::Value,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushTemplateReq {
    // ... existing fields ...
    pub resolvers: Option<Vec<ResolverReferenceReq>>,
}
```

### Step 4: Update Client

File: `cyanregistry/src/http/client.rs`

The `push_template()` method already sends the full `TemplateReq` including resolvers if present. No changes needed if the mapper is updated correctly.

## Zinc API Reference

**Push Template:** `POST /api/v{version}/Template/push/{username}`

The request body now includes:

```json
{
  "name": "my-template",
  "resolvers": [
    {
      "resolver": "atomi/json-merger:1",
      "config": { "strategy": "deep-merge" },
      "files": ["package.json", "**/tsconfig.json"]
    }
  ]
}
```

## Dependencies

- None (standalone feature)

## Testing

- [ ] Unit test: Parse cyan.yaml with resolvers
- [ ] Unit test: Map resolvers to request model
- [ ] Integration test: Push template with resolvers

## Integration Points

- Templates pushed with resolvers will have `resolvers` available in `TemplateVersionRes`
- Enables Plan 3 to access resolver info during composition

## Implementation Checklist

### cyanregistry/src/cli/models/template_config.rs

- [ ] Add `resolvers: Option<Vec<CyanResolverFileConfig>>` to `CyanTemplateFileConfig`
- [ ] Create `CyanResolverFileConfig` struct

### cyanregistry/src/domain/config/template_config.rs

- [ ] Add `resolvers: Option<Vec<ResolverConfig>>` to `TemplateConfig`
- [ ] Create `ResolverConfig` struct

### cyanregistry/src/cli/mapper.rs

- [ ] Add `resolver_config_mapper()` function
- [ ] Update `template_config_mapper()` to include resolvers

### cyanregistry/src/http/models/template_req.rs

- [ ] Add `ResolverReferenceReq` struct
- [ ] Add `resolvers: Option<Vec<ResolverReferenceReq>>` to `PushTemplateReq`

### cyanregistry/src/http/mapper.rs

- [ ] Add mapper for `ResolverConfig` → `ResolverReferenceReq`
- [ ] Update `template_req_with_properties_mapper()` to include resolvers

---

## Non-Functional Requirements

### Quality Gates (must pass before commit)

- [ ] `cargo test --workspace` passes
- [ ] `pre-commit run --all` passes
- [ ] `pls build` succeeds

### Documentation

Follow pattern in `docs/developer/`:

- [ ] Update `docs/developer/concepts/01-template.md` - Add resolver section
- [ ] Update `docs/developer/surfaces/cli/01-push.md` - Document resolver field in cyan.yaml
- [ ] Include mermaid diagrams for data flow

### Code Quality

- [ ] **3-layer separation**: CLI models → Domain config → HTTP models
- [ ] **No de-duplication in mappers** - each layer has single responsibility:
  - `cli/mapper.rs`: Parse YAML → Domain config (resolver reference string parsing)
  - `http/mapper.rs`: Domain config → HTTP request (direct field mapping)
- [ ] **Complexity check**: Follow existing plugin/processor pattern exactly
- [ ] **Elegance check**: Reuse existing `plugin_reference_mapper` pattern for resolver reference parsing

### Testing Strategy

- **Unit tests**:
  - YAML parsing with resolvers
  - Mapper functions (CLI → Domain → HTTP)
- **Integration tests**: Deferred to E2E (external Zinc endpoint dependency)
- **E2E tests**: Update existing templates to include resolvers

### E2E Test Setup

Update existing templates to test resolver references:

1. **Modify existing template configs**:

   - `e2e/template1/cyan.yaml` - Add resolver reference to `json-merger:1`
   - `e2e/template2/cyan.yaml` - Add resolver reference to `line-merger:1`

2. **E2E execution order** (in `e2e/e2e.sh`):

   ```
   1. Push resolvers (Plan 1)
   2. Push templates with resolver refs (Plan 2)
   ```

3. **Resolver reference format** in cyan.yaml:
   ```yaml
   resolvers:
     - resolver: 'cyane2e/json-merger:1'
       config:
         strategy: 'deep-merge'
       files:
         - 'package.json'
   ```

**E2E does NOT require evidence** - just follow the existing pattern.

---

## E2E Files to Modify

| File                      | Changes                                    |
| ------------------------- | ------------------------------------------ |
| `e2e/template1/cyan.yaml` | Add `resolvers` field with json-merger ref |
| `e2e/template2/cyan.yaml` | Add `resolvers` field with line-merger ref |
