# Task Spec: Allow Layerer to use resolver to resolve conflict

**Ticket:** CU-86ewrbrb0
**Component:** Iridium (cyancoordinator + cyanregistry)
**Parent:** CU-86ewr9nen (Resolver system)
**Blocking dependency:** CU-86ewuf8zx ([Zn] Add resolver config and glob patterns to registry API models)

## Summary

Implement resolver support in three parts:

1. **Resolver Push** (cyanregistry): Standalone resolver push to Zinc registry
2. **Template Push with Resolvers** (cyanregistry): Extend template push to include resolvers from `cyan.yaml`
3. **Conflict File Resolver** (cyancoordinator): Integrate resolver-based conflict resolution into VFS layering

## Background

### Resolvers

- Declared per-template in `cyan.yaml` under `resolvers:` field
- Unique instances: same resolver name + different config = different resolver
- Stateless HTTP services running on port 5553 (Helium SDK)
- Warmed by Boron alongside templates

### cyan.yaml Structure

```yaml
# Template's cyan.yaml
name: my-template
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

### Helium Resolver API (Port 5553)

```python
# ResolverInput (from Helium SDK)
class ResolverInput:
    config: Dict[str, Any]
    files: list[ResolvedFile]  # path, content, origin (template, layer)

# ResolverOutput
class ResolverOutput:
    path: str
    content: str
```

### Boron Proxy

- Transparent proxy to resolver containers
- Endpoint: `POST /proxy/resolver/{cyan_id}/api/resolve`
- Resolver containers warmed during `warm_template()`

### Zinc API Endpoints

| Endpoint                                        | Purpose                                |
| ----------------------------------------------- | -------------------------------------- |
| `POST /api/v{version}/Resolver/push/{username}` | Push standalone resolver               |
| `POST /api/v{version}/Template/push/{username}` | Push template (now includes resolvers) |

---

# Part 1: Resolver Push (cyanregistry)

## Goal

Enable pushing resolvers to Zinc registry as standalone artifacts.

## Zinc API Reference

**Push Resolver:** `POST /api/v{version}/Resolver/push/{username}`

**ResolverReq:**

```json
{
  "name": "json-merger",
  "project": "atomi",
  "source": "github.com/atomi/json-merger",
  "email": "team@atomi.com",
  "tags": ["json", "merge"],
  "description": "Deep merge JSON files",
  "readme": "# JSON Merger\n...",
  "versionDescription": "Initial version",
  "dockerReference": "atomi/json-merger",
  "dockerTag": "1.0.0"
}
```

**Response:** `ResolverVersionPrincipalRes`

## Files to Modify/Create

### Modify

- `cyanregistry/src/http/client.rs` - Add `push_resolver()` method
- `cyanregistry/src/http/models/mod.rs` - Export new resolver models

### Create

- `cyanregistry/src/http/models/resolver_req.rs` - `ResolverReq`
- `cyanregistry/src/http/models/resolver_res.rs` - `ResolverVersionPrincipalRes`, `ResolverPrincipalRes`, `ResolverVersionRes`

## Implementation Checklist

### cyanregistry/src/http/models/resolver_req.rs

- [ ] Create `ResolverReq` struct with fields: name, project, source, email, tags, description, readme, versionDescription, dockerReference, dockerTag

### cyanregistry/src/http/models/resolver_res.rs

- [ ] Create `ResolverPrincipalRes` struct
- [ ] Create `ResolverVersionPrincipalRes` struct
- [ ] Create `ResolverVersionRes` struct

### cyanregistry/src/http/models/mod.rs

- [ ] Export `resolver_req` and `resolver_res` modules

### cyanregistry/src/http/client.rs

- [ ] Add `push_resolver()` method to `CyanRegistryClient`
- [ ] Follow same pattern as `push_template()`, `push_plugin()`, `push_processor()`

---

# Part 2: Template Push with Resolvers (cyanregistry)

## Goal

Extend template push to include resolver declarations from `cyan.yaml`. When pushing a template, also push its resolver associations so they're available at runtime.

## cyan.yaml Resolver Configuration

```yaml
resolvers:
  - resolver: 'atomi/json-merger:1' # resolver reference (username/name:version)
    config:
      strategy: 'deep-merge' # JSON config passed to resolver
    files:
      - 'package.json' # Glob patterns for which files this resolver handles
      - '**/tsconfig.json'
```

## Zinc API Reference

**Push Template:** `POST /api/v{version}/Template/push/{username}`

The `PushTemplateReq` now needs to include resolvers:

```json
{
  "name": "my-template",
  "project": "atomi",
  "source": "...",
  "email": "...",
  "tags": [...],
  "description": "...",
  "readme": "...",
  "versionDescription": "...",
  "properties": {
    "blobDockerReference": "...",
    "blobDockerTag": "...",
    "templateDockerReference": "...",
    "templateDockerTag": "..."
  },
  "plugins": [...],
  "processors": [...],
  "resolvers": [
    {
      "resolverReference": "atomi/json-merger",
      "resolverVersion": 1,
      "config": { "strategy": "deep-merge" },
      "files": ["package.json", "**/tsconfig.json"]
    }
  ]
}
```

**Note:** The resolver must already exist in Zinc before being referenced in a template push.

## Files to Modify/Create

### Modify

- `cyanregistry/src/cli/models/template_config.rs` - Add `resolvers` field to template config
- `cyanregistry/src/cli/models/resolver_config.rs` - Create resolver config model
- `cyanregistry/src/http/models/template_req.rs` - Add `resolvers` to `TemplateReq`
- `cyanregistry/src/http/models/mod.rs` - Export resolver reference models
- `cyanregistry/src/http/client.rs` - Update `push_template()` to include resolvers
- `cyanregistry/src/cli/mapper.rs` - Map resolver configs from YAML to request

### Create

- `cyanregistry/src/http/models/resolver_reference_req.rs` - `ResolverReferenceReq` for template push

## Implementation Checklist

### cyanregistry/src/cli/models/resolver_config.rs (NEW)

- [ ] Create `CyanResolverFileConfig` struct matching cyan.yaml resolver format
- [ ] Fields: resolver (reference string), config (JSON), files (glob patterns)

### cyanregistry/src/cli/models/template_config.rs

- [ ] Add `resolvers: Vec<CyanResolverFileConfig>` to `CyanTemplateFileConfig`

### cyanregistry/src/http/models/resolver_reference_req.rs (NEW)

- [ ] Create `ResolverReferenceReq` struct
- [ ] Fields: resolverReference, resolverVersion, config, files

### cyanregistry/src/http/models/template_req.rs

- [ ] Add `resolvers: Option<Vec<ResolverReferenceReq>>` to `TemplateReq`

### cyanregistry/src/http/models/mod.rs

- [ ] Export `resolver_reference_req` module

### cyanregistry/src/cli/mapper.rs

- [ ] Add resolver config mapper function
- [ ] Map `CyanResolverFileConfig` to `ResolverReferenceReq`

### cyanregistry/src/http/client.rs

- [ ] Update `push_template()` to include resolvers in request
- [ ] Update `push_template_without_properties()` to handle resolvers

---

# Part 3: Conflict File Resolver (cyancoordinator)

## Two Layering Contexts

### 1. Vertical Layering (Dependency Tree)

- When resolving dependencies for a root template
- **Collect resolvers from ALL templates in the dependency tree**

### 2. Horizontal Layering (Folder Composition)

- When merging all templates used for the current folder
- **Collect resolvers ONLY from the root templates being merged**

## Resolver Consensus Algorithm

**Key Insight:** "No resolver" is a valid resolver choice and must participate in consensus.

### Consensus Rules

| All Variations Agree?                    | Action                            |
| ---------------------------------------- | --------------------------------- |
| ALL = same resolver instance             | ✅ Use that resolver              |
| ALL = none                               | ⚠️ LWW (`lww_all_no_resolver`)    |
| MIXED (some X, some none)                | ⚠️ LWW (`lww_no_consensus`)       |
| MIXED (X vs Y, different resolvers)      | ⚠️ LWW (`lww_ambiguous_resolver`) |
| MIXED (X with configA vs X with configB) | ⚠️ LWW (`lww_ambiguous_resolver`) |

### Resolution Types

| Type                     | Meaning                              | When Used                                        |
| ------------------------ | ------------------------------------ | ------------------------------------------------ |
| `resolver`               | Resolver successfully resolved       | All variations agree on same resolver instance   |
| `lww_all_no_resolver`    | LWW - no resolver configured         | All variations have no resolver for this file    |
| `lww_no_consensus`       | LWW - some have resolver, some don't | Mixed: some variations have resolver, some don't |
| `lww_ambiguous_resolver` | LWW - multiple different resolvers   | All have resolvers but they differ               |

## Helium SDK Compatibility

Resolver input/output must match Helium SDK:

```rust
// ResolverInput - matches Helium SDK
pub struct ResolverInput {
    pub config: serde_json::Value,
    pub files: Vec<ResolverFile>,
}

pub struct ResolverFile {
    pub path: String,
    pub content: String,
    pub origin: FileOrigin,  // template name + layer index
}

pub struct FileOrigin {
    pub template: String,
    pub layer: i32,
}

// ResolverOutput - matches Helium SDK
pub struct ResolverOutput {
    pub path: String,
    pub content: String,
}
```

## Files to Modify/Create

### Modify

- `cyanregistry/src/http/models/template_res.rs` - Add `resolvers` field to `TemplateVersionRes`
- `cyancoordinator/Cargo.toml` - Add `glob = "0.3"` dependency
- `cyancoordinator/src/lib.rs` - Export `conflict_file_resolver` module
- `cyancoordinator/src/client.rs` - Add `resolve_files()` method
- `cyancoordinator/src/operations/composition/layerer.rs` - Add `ResolverAwareLayerer`
- `cyancoordinator/src/operations/composition/operator.rs` - Integrate resolver collection
- `cyancoordinator/src/state/models.rs` - Add `file_conflicts` to `CyanState`

### Create

- `cyancoordinator/src/conflict_file_resolver/mod.rs`
- `cyancoordinator/src/conflict_file_resolver/models.rs`
- `cyancoordinator/src/conflict_file_resolver/registry.rs`
- `cyancoordinator/src/conflict_file_resolver/consensus.rs`

## Implementation Checklist

### cyanregistry/src/http/models/template_res.rs

- [ ] Add `resolvers: Vec<TemplateVersionResolverRes>` to `TemplateVersionRes`
- [ ] Create `TemplateVersionResolverRes` struct with: id, version, created_at, description, docker_reference, docker_tag, config, files

### cyancoordinator/Cargo.toml

- [ ] Add `glob = "0.3"` dependency

### cyancoordinator/src/conflict_file_resolver/models.rs

- [ ] Create `ResolverInstance` struct
- [ ] Create `ResolverChoice` enum (None/Some)
- [ ] Create `ResolverInput`, `ResolverFile`, `FileOrigin` (match Helium SDK)
- [ ] Create `ResolverOutput` (match Helium SDK)
- [ ] Create `FileConflictEntry` for state tracking
- [ ] Create `ConflictResolution` enum

### cyancoordinator/src/conflict_file_resolver/registry.rs

- [ ] Create `ConflictFileResolverRegistry` struct
- [ ] Implement `get_resolver_choice(template_id, path) -> ResolverChoice`
- [ ] Implement glob pattern matching

### cyancoordinator/src/conflict_file_resolver/consensus.rs

- [ ] Create `ConsensusResult` enum
- [ ] Implement `determine_consensus()` function
- [ ] Handle all 4 cases: Agreed, AllNone, NoConsensus, Ambiguous

### cyancoordinator/src/conflict_file_resolver/mod.rs

- [ ] Export all public types

### cyancoordinator/src/client.rs

- [ ] Add `resolve_files()` method to `CyanCoordinatorClient`
- [ ] Endpoint: `POST {endpoint}/proxy/resolver/{cyan_id}/api/resolve`

### cyancoordinator/src/state/models.rs

- [ ] Add `file_conflicts: Vec<FileConflictEntry>` to `CyanState`

### cyancoordinator/src/operations/composition/layerer.rs

- [ ] Create `ResolverAwareLayerer` struct
- [ ] Implement `VfsLayerer` trait
- [ ] Implement file grouping by path
- [ ] Implement consensus-based resolution
- [ ] Implement LWW fallback with tracking

### cyancoordinator/src/operations/composition/operator.rs

- [ ] Collect resolvers from templates (scope-aware: vertical vs horizontal)
- [ ] Build `ConflictFileResolverRegistry`
- [ ] Use `ResolverAwareLayerer` for merging
- [ ] Write `file_conflicts` to `.cyan_state.yaml`

---

## Edge Cases

| Scenario                         | Resolution Type          | Behavior              |
| -------------------------------- | ------------------------ | --------------------- |
| All agree on resolver X          | `resolver`               | Invoke resolver X     |
| All have no resolver             | `lww_all_no_resolver`    | LWW                   |
| 4 have X, 1 has none             | `lww_no_consensus`       | LWW (no consensus)    |
| 3 have X, 2 have Y               | `lww_ambiguous_resolver` | LWW (ambiguous)       |
| Same resolver, different configs | `lww_ambiguous_resolver` | LWW (config differs)  |
| Resolver HTTP fails              | N/A                      | **Abort composition** |
| Binary files                     | `resolver`               | Content as base64     |

## Error Handling (Fail Fast)

**Resolver failures ABORT the composition** — no silent fallback.

## Acceptance Criteria

1. Can push resolvers to Zinc registry (Part 1)
2. Can push templates with resolver declarations (Part 2)
3. Vertical layering collects resolvers from ALL templates in dependency tree (Part 3)
4. Horizontal layering collects resolvers ONLY from root templates being merged (Part 3)
5. Consensus requires ALL variations to agree on the SAME resolver instance (Part 3)
6. "No resolver" is treated as a valid choice in consensus (Part 3)
7. Four distinct resolution types tracked (Part 3)
8. Resolver failures abort composition (Part 3)
9. Backward compatible when no resolvers declared (Part 3)

## Testing Requirements

### Part 1: Resolver Push

- [ ] Unit test: `push_resolver()` with valid request
- [ ] Unit test: `push_resolver()` with API error

### Part 2: Template Push with Resolvers

- [ ] Unit test: Parse resolver config from YAML
- [ ] Unit test: `push_template()` includes resolvers in request

### Part 3: Conflict File Resolver

- [ ] Unit tests for consensus algorithm (all 4 cases)
- [ ] Unit tests for glob pattern matching
- [ ] Integration test: All agree on same resolver → resolved
- [ ] Integration test: All no resolver → LWW
- [ ] Integration test: Mixed consensus → LWW
- [ ] Integration test: Resolver HTTP failure → abort
- [ ] Integration test: `file_conflicts` tracking

---

## Non-Functional Requirements

### Quality Gates (all plans)

- [ ] `cargo test --workspace` passes
- [ ] `pre-commit run --all` passes
- [ ] `pls build` succeeds

### Code Quality

- **3-layer separation**: CLI models → Domain config → HTTP models (no mixing)
- **No de-duplication in mappers**: Each layer has single responsibility
- **Complexity check**: Keep implementations simple, no over-engineering
- **Elegance check**: Follow existing patterns exactly

### Documentation

- Follow pattern in `docs/developer/`
- Include mermaid diagrams for algorithms and flows
- Update relevant module and feature docs

### E2E Testing Strategy

- **E2E focus**: Create + push only
- **No evidence required**: Follow existing pattern in `e2e/` folder
- **Push order**: Can emulate v1/v2 by pushing same artifact twice
- **Username**: `cyane2e`
- **Integration tests**: Defer to E2E (external endpoint dependencies)
