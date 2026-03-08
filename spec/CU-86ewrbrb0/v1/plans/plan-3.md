# Plan 3: Conflict File Resolver (cyancoordinator)

## Goal

Integrate resolver-based conflict resolution into the VFS layering phase. When multiple templates produce the same file, use resolvers when all variations agree, otherwise fall back to LWW with tracking.

## Scope

- Update `TemplateVersionRes` to include `resolvers` field
- Add `resolve_files()` to `CyanCoordinatorClient`
- Create `conflict_file_resolver` module with consensus algorithm
- Create `ResolverAwareLayerer`
- Track conflict resolutions in `.cyan_state.yaml`

## Dependencies

- **Plan 1 (Resolver Push)** must be complete for E2E testing
- **Plan 2 (Template Push with Resolvers)** must be complete for E2E testing
- **Zinc API**: `TemplateVersionRes.resolvers[]` field (CU-86ewuf8zx)
- **Boron Proxy**: `POST /proxy/resolver/{cyan_id}/api/resolve`

## Helium Resolver API Reference

```python
# Input (matches Helium SDK)
class ResolverInput:
    config: Dict[str, Any]
    files: list[ResolvedFile]

class ResolvedFile:
    path: str
    content: str
    origin: FileOrigin

class FileOrigin:
    template: str
    layer: int

# Output
class ResolverOutput:
    path: str
    content: str
```

## Files to Modify

| File                                                     | Changes                                |
| -------------------------------------------------------- | -------------------------------------- |
| `cyanregistry/src/http/models/template_res.rs`           | Add `resolvers` field                  |
| `cyancoordinator/Cargo.toml`                             | Add `glob = "0.3"` dependency          |
| `cyancoordinator/src/lib.rs`                             | Export `conflict_file_resolver` module |
| `cyancoordinator/src/client.rs`                          | Add `resolve_files()` method           |
| `cyancoordinator/src/operations/composition/layerer.rs`  | Add `ResolverAwareLayerer`             |
| `cyancoordinator/src/operations/composition/operator.rs` | Integrate resolver collection          |
| `cyancoordinator/src/state/models.rs`                    | Add `file_conflicts` field             |

## Files to Create

| File                                                      | Purpose                                                                           |
| --------------------------------------------------------- | --------------------------------------------------------------------------------- |
| `cyancoordinator/src/conflict_file_resolver/mod.rs`       | Module exports                                                                    |
| `cyancoordinator/src/conflict_file_resolver/models.rs`    | `ResolverInstance`, `ResolverChoice`, `ResolverInput/Output`, `FileConflictEntry` |
| `cyancoordinator/src/conflict_file_resolver/registry.rs`  | `ConflictFileResolverRegistry` with glob matching                                 |
| `cyancoordinator/src/conflict_file_resolver/consensus.rs` | Consensus algorithm                                                               |

## Implementation Order

### Phase 1: Models & Registry

#### 1.1 Add `glob` dependency

File: `cyancoordinator/Cargo.toml`

- [ ] Add `glob = "0.3"` to dependencies

#### 1.2 Update TemplateVersionRes

File: `cyanregistry/src/http/models/template_res.rs`

- [ ] Add `resolvers: Vec<TemplateVersionResolverRes>` to `TemplateVersionRes`
- [ ] Create `TemplateVersionResolverRes`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateVersionResolverRes {
    pub id: String,
    pub version: i64,
    pub created_at: String,
    pub description: Option<String>,
    pub docker_reference: String,
    pub docker_tag: String,
    pub config: serde_json::Value,
    pub files: Vec<String>,  // Glob patterns
}
```

#### 1.3 Create Resolver Models

File: `cyancoordinator/src/conflict_file_resolver/models.rs`

- [ ] `ResolverInstance` - unique resolver (id, docker_reference, docker_tag, config, file_patterns)
- [ ] `ResolverChoice` enum - `None` or `Some(ResolverInstance)`
- [ ] `FileOrigin` - template metadata (template_id, template_version, layer)
- [ ] `ResolverFile` - single file variation (path, content, origin)
- [ ] `ResolverInput` - matches Helium SDK (config, files)
- [ ] `ResolverOutput` - from resolver (path, content)
- [ ] `ConflictResolution` enum
- [ ] `FileConflictEntry` - for state tracking

#### 1.4 Create Registry

File: `cyancoordinator/src/conflict_file_resolver/registry.rs`

- [ ] `ConflictFileResolverRegistry` struct
- [ ] Method: `register(template_id, resolvers: Vec<ResolverInstance>)`
- [ ] Method: `get_resolver_choice(template_id, path) -> ResolverChoice`
- [ ] Use `glob` crate for pattern matching

#### 1.5 Create Consensus Algorithm

File: `cyancoordinator/src/conflict_file_resolver/consensus.rs`

- [ ] `ConsensusResult` enum:
  - `Agreed(ResolverInstance)`
  - `AllNone`
  - `NoConsensus { with_resolver, without_resolver }`
  - `Ambiguous { resolvers }`
- [ ] Function: `determine_consensus(choices: Vec<(TemplateInfo, ResolverChoice)>) -> ConsensusResult`

#### 1.6 Create Module Exports

File: `cyancoordinator/src/conflict_file_resolver/mod.rs`

- [ ] Export all public types

### Phase 2: Client & State

#### 2.1 Add resolve_files() to Client

File: `cyancoordinator/src/client.rs`

- [ ] Add `resolve_files(cyan_id: &str, input: &ResolverInput) -> Result<ResolverOutput, ...>`
- [ ] Endpoint: `POST {endpoint}/proxy/resolver/{cyan_id}/api/resolve`

#### 2.2 Update CyanState

File: `cyancoordinator/src/state/models.rs`

- [ ] Add `file_conflicts: Vec<FileConflictEntry>` to `CyanState`
- [ ] Add `FileConflictEntry` struct

### Phase 3: Layerer Implementation

#### 3.1 Create ResolverAwareLayerer

File: `cyancoordinator/src/operations/composition/layerer.rs`

- [ ] Create `ResolverAwareLayerer` struct
- [ ] Implement `VfsLayerer` trait
- [ ] Algorithm:
  1. Group files by path, track source template
  2. For paths with 1 variation: add directly
  3. For paths with 2+ variations:
     a. Get `ResolverChoice` for each
     b. Call `determine_consensus()`
     c. Based on result: resolve or LWW with tracking

### Phase 4: Integration

#### 4.1 Update CompositionOperator

File: `cyancoordinator/src/operations/composition/operator.rs`

- [ ] **Resolver collection scope:**
  - Vertical: ALL templates in dependency tree
  - Horizontal: ONLY root templates being merged
- [ ] After executing templates:
  1. Collect resolvers based on context
  2. Build `ConflictFileResolverRegistry`
  3. Create `ResolverAwareLayerer`
  4. Call `layer_merge()`
  5. Write `file_conflicts` to `.cyan_state.yaml`

## Resolver Consensus Algorithm

```text
Input: Vec<(TemplateInfo, ResolverChoice)>

1. Extract all unique resolver choices
2. If all choices are None → return AllNone
3. If any choice is None and any is Some → return NoConsensus
4. If multiple different Some values → return Ambiguous
5. If all Some values identical (same ref + config) → return Agreed
```

**Resolver Identity:** Same if:

- Same `docker_reference`
- Same `docker_tag`
- Same `config` (JSON equality)

## Conflict Tracking Structure

```yaml
# .cyan_state.yaml
file_conflicts:
  - path: 'package.json'
    resolution: 'resolver'
    resolver_used:
      id: 'uuid'
      docker_reference: 'atomi/json-merger'
      docker_tag: '1'
      config: { 'strategy': 'deep' }
    variations:
      - template_id: 'a'
      - template_id: 'b'

  - path: '.gitignore'
    resolution: 'lww_no_consensus'
    with_resolver:
      - template_id: 'a'
        docker_reference: 'atomi/line-merger:1'
    without_resolver:
      - template_id: 'b'
    winner_template: 'b'
```

## Edge Cases

- [ ] Empty VFS list → return empty VFS
- [ ] Single VFS → no conflicts
- [ ] No resolvers declared → pure LWW
- [ ] Resolver HTTP failure → abort composition
- [ ] Binary files → base64 content

## Error Handling

**Resolver failures ABORT composition:**

- HTTP 5xx → abort
- Timeout → abort
- Invalid JSON → abort
- 4xx → abort

## Implementation Checklist

### cyanregistry

- [ ] Add `resolvers` field to `TemplateVersionRes`
- [ ] Create `TemplateVersionResolverRes`

### cyancoordinator/Cargo.toml

- [ ] Add `glob = "0.3"`

### cyancoordinator/src/conflict_file_resolver/models.rs

- [ ] Create all models

### cyancoordinator/src/conflict_file_resolver/registry.rs

- [ ] Create `ConflictFileResolverRegistry`
- [ ] Implement `register()`, `get_resolver_choice()`
- [ ] Implement glob matching

### cyancoordinator/src/conflict_file_resolver/consensus.rs

- [ ] Create `ConsensusResult` enum
- [ ] Implement `determine_consensus()`

### cyancoordinator/src/conflict_file_resolver/mod.rs

- [ ] Export all types

### cyancoordinator/src/client.rs

- [ ] Add `resolve_files()`

### cyancoordinator/src/state/models.rs

- [ ] Add `file_conflicts` to `CyanState`

### cyancoordinator/src/operations/composition/layerer.rs

- [ ] Create `ResolverAwareLayerer`
- [ ] Implement conflict detection and resolution

### cyancoordinator/src/operations/composition/operator.rs

- [ ] Collect resolvers with scope awareness
- [ ] Integrate layerer
- [ ] Write `file_conflicts` to state

## Testing

- [ ] Unit tests for consensus (all 4 cases)
- [ ] Unit tests for glob matching
- [ ] Integration: resolver agreement → resolved
- [ ] Integration: no resolver → LWW
- [ ] Integration: mixed consensus → LWW
- [ ] Integration: resolver failure → abort
- [ ] Integration: `file_conflicts` tracking

---

## Non-Functional Requirements

### Quality Gates (must pass before commit)

- [ ] `cargo test --workspace` passes
- [ ] `pre-commit run --all` passes
- [ ] `pls build` succeeds

### Documentation

Follow pattern in `docs/developer/`:

- [ ] Create `docs/developer/algorithms/04-conflict-resolution.md` - Consensus algorithm
- [ ] Update `docs/developer/features/03-vfs-layering.md` - Add resolver-aware layering
- [ ] Update `docs/developer/modules/02-cyancoordinator.md` - Add conflict_file_resolver module
- [ ] Include mermaid sequence diagrams for resolution flow

### Code Quality

- [ ] **3-layer separation**: No mixing of concerns between models/registry/consensus
- [ ] **No duplication**: Each struct has single purpose
- [ ] **Complexity check**:
  - Consensus algorithm should be < 50 lines
  - Registry should use standard glob crate patterns
  - Layerer should delegate to consensus module
- [ ] **Elegance check**: Match existing layerer patterns, use trait-based design

### Testing Strategy

- **Unit tests** (in-module):
  - Consensus algorithm (all 4 cases)
  - Glob pattern matching
  - Resolver identity equality
- **Integration tests**: Deferred to E2E (requires running Boron proxy + resolver containers)
- **E2E tests**: Composition with conflict resolution

### E2E Test Setup

**Requires Plan 1 and Plan 2 to be complete first** (resolvers + templates with resolvers pushed).

Create test templates that produce conflicts:

1. **Create conflict test templates**:

   - `e2e/test-conflict-a/` - Template A with json-merger resolver
   - `e2e/test-conflict-b/` - Template B with json-merger resolver
   - Both output `package.json` with different content

2. **Test scenarios**:

   - **All agree**: A + B both declare same resolver → resolved
   - **All none**: A + B no resolvers → LWW
   - **Mixed**: A has resolver, B doesn't → LWW with tracking
   - **Ambiguous**: A has json-merger, B has line-merger → LWW with tracking

3. **E2E execution order** (in `e2e/e2e.sh`):

   ```
   1. Push resolvers (Plan 1)
   2. Push templates with resolvers (Plan 2)
   3. Push conflict test templates (Plan 3)
   4. Run composition and verify .cyan_state.yaml
   ```

4. **Verification**:
   - Check `.cyan_state.yaml` contains `file_conflicts` entries
   - Verify resolution type matches expected behavior

**E2E does NOT require evidence** - just follow the existing pattern.

---

## E2E Files to Create

| File                                        | Purpose                  |
| ------------------------------------------- | ------------------------ |
| `e2e/test-conflict-a/cyan.yaml`             | Template A with resolver |
| `e2e/test-conflict-a/template/package.json` | Conflict file            |
| `e2e/test-conflict-b/cyan.yaml`             | Template B with resolver |
| `e2e/test-conflict-b/template/package.json` | Conflict file            |

## E2E Files to Modify

| File         | Changes                               |
| ------------ | ------------------------------------- |
| `e2e/e2e.sh` | Add conflict test template publishing |
