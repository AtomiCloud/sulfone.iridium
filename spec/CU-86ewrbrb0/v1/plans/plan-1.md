# Plan 1: Resolver Push (cyanregistry)

## Goal

Enable pushing resolvers to Zinc registry as standalone artifacts.

## Scope

- Add resolver request/response models to cyanregistry
- Add `push_resolver()` method to `CyanRegistryClient`
- Follow same patterns as existing `push_template()`, `push_plugin()`, `push_processor()`

## Files to Modify

| File                                  | Changes                        |
| ------------------------------------- | ------------------------------ |
| `cyanregistry/src/http/models/mod.rs` | Export resolver request models |
| `cyanregistry/src/http/client.rs`     | Add `push_resolver()` method   |

## Files to Create

| File                                           | Purpose                 |
| ---------------------------------------------- | ----------------------- |
| `cyanregistry/src/http/models/resolver_req.rs` | `PushResolverReq` model |

## Implementation Order

### Step 1: Create Resolver Request Models

File: `cyanregistry/src/http/models/resolver_req.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushResolverReq {
    pub name: String,
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_description: Option<String>,
    pub docker_reference: String,
    pub docker_tag: String,
}
```

### Step 2: Export in mod.rs

File: `cyanregistry/src/http/models/mod.rs`

- [ ] Add `pub mod resolver_req;`
- [ ] Re-export `PushResolverReq`

### Step 3: Add push_resolver() to Client

File: `cyanregistry/src/http/client.rs`

Follow the exact pattern of `push_template()`:

```rust
pub fn push_resolver(
    &self,
    config_path: String,
    token: String,
    desc: String,
    docker_ref: String,
    docker_tag: String,
) -> Result<ResolverVersionPrincipalRes, Box<dyn Error + Send>> {
    // 1. Read YAML config from config_path
    // 2. Map to PushResolverReq
    // 3. POST to /api/v{version}/Resolver/push/{username}
    // 4. Return ResolverVersionPrincipalRes
}
```

**Note:** May need to create `ResolverVersionPrincipalRes` if not already defined.

## Zinc API Reference

**Endpoint:** `POST /api/v{version}/Resolver/push/{username}`

**Request (PushResolverReq):**

```json
{
  "name": "json-merger",
  "project": "atomi",
  "source": "github.com/atomi/resolvers",
  "email": "dev@atomi.com",
  "tags": ["json", "merge"],
  "description": "Deep merge JSON files",
  "readme": "# JSON Merger\n\nMerges JSON files...",
  "versionDescription": "Initial version",
  "dockerReference": "atomi/json-merger",
  "dockerTag": "1.0.0"
}
```

**Response:** `ResolverVersionPrincipalRes`

## Dependencies

- None (standalone feature)

## Testing

- [ ] Unit test: `push_resolver()` with valid request returns version
- [ ] Unit test: `push_resolver()` with API error returns error

## Integration Points

- This enables E2E tests to push resolvers before testing conflict resolution
- Used by CI/CD to publish resolver artifacts

## Implementation Checklist

### cyanregistry/src/http/models/resolver_req.rs

- [ ] Create `PushResolverReq` struct
- [ ] Add all fields with proper serde attributes
- [ ] Use `#[serde(rename_all = "camelCase")]`

### cyanregistry/src/http/models/mod.rs

- [ ] Add `pub mod resolver_req;`
- [ ] Re-export `PushResolverReq`

### cyanregistry/src/http/models/resolver_res.rs (if needed)

- [ ] Create `ResolverVersionPrincipalRes` if not exists
- [ ] Fields: id, version, created_at, description, docker_reference, docker_tag

### cyanregistry/src/http/client.rs

- [ ] Add `push_resolver_internal()` following existing pattern
- [ ] Add `push_resolver()` public method
- [ ] Use endpoint: `/api/v{version}/Resolver/push/{username}`
- [ ] Return `ResolverVersionPrincipalRes`

---

## Non-Functional Requirements

### Quality Gates (must pass before commit)

- [ ] `cargo test --workspace` passes
- [ ] `pre-commit run --all` passes
- [ ] `pls build` succeeds

### Documentation

Follow pattern in `docs/developer/`:

- [ ] Create `docs/developer/concepts/09-resolver.md` - Resolver concept overview
- [ ] Update `docs/developer/surfaces/cli/01-push.md` - Add resolver push CLI docs
- [ ] Include mermaid diagrams for flows

### Code Quality

- [ ] **3-layer separation**: CLI models → Domain config → HTTP models
- [ ] **No de-duplication in mappers** - each layer has single responsibility
- [ ] **Complexity check**: Keep it simple, no over-engineering
- [ ] **Elegance check**: Follow existing patterns exactly (see `push_plugin()`)

### Testing Strategy

- **Unit tests**: Model serialization, mapper functions
- **Integration tests**: Deferred to E2E (external Zinc endpoint dependency)
- **E2E tests**: Create + push only (see below)

### E2E Test Setup

Follow existing pattern in `e2e/` folder:

1. **Create artifact directories**:

   - `e2e/resolver1/` - First resolver (e.g., `json-merger`)
   - `e2e/resolver2/` - Second resolver (e.g., `line-merger`)

2. **Create publish script**: `e2e/publish-resolver.sh`

   - Follow pattern from `e2e/publish-plugin.sh`
   - Build Docker image, push to registry, call `cyanprint push resolver`

3. **Update `e2e/e2e.sh`**:

   - Add resolver publishing before templates
   - Push order: `resolver1` → `resolver1` (emulate v1 → v2)

4. **Push pattern for versioning**:
   - Same endpoint name, push twice to create v1 and v2
   - Username: `cyane2e`

**E2E does NOT require evidence** - just follow the existing pattern.

---

## E2E Files to Create

| File                       | Purpose                       |
| -------------------------- | ----------------------------- |
| `e2e/resolver1/cyan.yaml`  | Resolver config (json-merger) |
| `e2e/resolver1/Dockerfile` | Docker build for resolver     |
| `e2e/resolver1/README.md`  | Resolver readme               |
| `e2e/resolver2/cyan.yaml`  | Resolver config (line-merger) |
| `e2e/resolver2/Dockerfile` | Docker build for resolver     |
| `e2e/resolver2/README.md`  | Resolver readme               |
| `e2e/publish-resolver.sh`  | Publish script                |

## E2E Files to Modify

| File         | Changes                       |
| ------------ | ----------------------------- |
| `e2e/e2e.sh` | Add resolver publishing steps |
