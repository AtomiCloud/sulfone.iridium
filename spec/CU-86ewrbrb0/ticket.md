# Ticket: CU-86ewrbrb0

- **Type**: task
- **Status**: backlog
- **URL**: https://app.clickup.com/t/86ewrbrb0
- **Parent**: CU-86ewr9nen

## Description

Iridium handles the VFS (Virtual File System) merge phase. This sub-plan describes how to integrate resolver invocation during the layering process to handle file conflicts.

Constraints

No Type Constraints:
The iridium codebase does NOT constrain artifact types to a fixed set. Types are string-based and handled via pattern matching. Adding resolvers requires no changes to validators or type enums.

References

| What                 | Reference File                                         |
| -------------------- | ------------------------------------------------------ |
| VFS layerer trait    | cyancoordinator/src/operations/composition/layerer.rs  |
| Default layerer      | DefaultVfsLayerer in layerer.rs                        |
| VFS model            | cyancoordinator/src/fs/vfs.rs                          |
| Composition operator | cyancoordinator/src/operations/composition/operator.rs |
| Registry client      | cyanregistry/src/http/client.rs                        |

Design

Key Concept: Unique Resolvers

1 resolver = 1 unique resolver instance

Even if two resolvers have the same username/name:version, if they have different configs, they are considered 2 different resolvers.

Example:

```yaml
resolvers:
  - resolver: 'atomi/json-merger:1'
    config: { strategy: 'deep-merge' } # Resolver A
    files: ['package.json']
  - resolver: 'atomi/json-merger:1'
    config: { strategy: 'shallow' } # Resolver B (different!)
    files: ['tsconfig.json']
```

These are 2 unique resolvers, not 1.

VFS Interface (Unchanged)

The VFS interface stays simple - just path -> content:

```rust
pub struct VirtualFileSystem {
    files: HashMap<PathBuf, Vec<u8>>,
}
```

No need to break VFS. The layerer gets context separately.

Directives

1. Resolver Models

Create cyancoordinator/src/resolver/models.rs:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Unique resolver instance (includes config for uniqueness)
pub struct ResolverInstance {
    pub id: String,              // Unique ID for this instance
    pub resolver_ref: String,    // "username/name:version"
    pub cyan_id: String,         // Container cyan ID
    pub config: serde_json::Value,
    pub file_patterns: Vec<String>,  // Glob patterns
}

/// Input to resolver
#[derive(Serialize)]
pub struct ResolverInput {
    pub config: serde_json::Value,
    pub files: Vec<ResolverFile>,
}

#[derive(Serialize)]
pub struct ResolverFile {
    pub path: String,
    pub content: String,  // Base64 or UTF-8 string
}

/// Output from resolver
#[derive(Deserialize)]
pub struct ResolverOutput {
    pub path: String,
    pub content: String,
}
```

2. Resolver Client

Create cyancoordinator/src/resolver/client.rs:

```rust
use async_trait::async_trait;

#[async_trait]
pub trait ResolverClient: Send + Sync {
    async fn resolve(
        &self,
        cyan_id: &str,
        input: ResolverInput,
    ) -> Result<ResolverOutput, ResolverError>;
}

pub struct HttpResolverClient {
    base_url: String,
    // ... http client
}

// POST to {base_url}/proxy/resolver/{cyan_id}/api/resolve
```

3. Resolver Registry

Create cyancoordinator/src/resolver/registry.rs:

```rust
use glob::Pattern;

pub struct ResolverRegistry {
    resolvers: Vec<ResolverInstance>,
}

impl ResolverRegistry {
    pub fn new(resolvers: Vec<ResolverInstance>) -> Self {
        Self { resolvers }
    }

    /// Find exactly one matching resolver for a file path
    /// Returns None if 0 matches or >1 matches (ambiguous)
    pub fn find_unique_resolver(&self, path: &Path) -> Option<&ResolverInstance> {
        let matches: Vec<_> = self.resolvers
            .iter()
            .filter(|r| self.matches_pattern(path, &r.file_patterns))
            .collect();

        if matches.len() == 1 {
            Some(matches[0])
        } else {
            None  // 0 or >1 matches → ambiguous, use LWW
        }
    }

    fn matches_pattern(&self, path: &Path, patterns: &[String]) -> bool {
        let path_str = path.to_string_lossy();
        patterns.iter().any(|p| {
            Pattern::new(p)
                .map(|pat| pat.matches(&path_str))
                .unwrap_or(false)
        })
    }
}
```

4. Update Layerer

In layerer.rs, create a new layerer:

```rust
pub struct ResolverAwareLayerer {
    resolver_client: Arc<dyn ResolverClient>,
    resolver_registry: ResolverRegistry,
}

impl ResolverAwareLayerer {
    pub fn new(
        resolver_client: Arc<dyn ResolverClient>,
        resolver_registry: ResolverRegistry,
    ) -> Self {
        Self { resolver_client, resolver_registry }
    }

    pub async fn layer_merge(
        &self,
        vfs_list: &[VirtualFileSystem],
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        // 1. Group files by path, detect conflicts
        let (non_conflicts, conflicts) = self.group_files_by_conflict(vfs_list);

        // 2. Start result with non-conflicting files (LWW)
        let mut result = VirtualFileSystem::new();
        for (path, content) in non_conflicts {
            result.add_file(path, content);
        }

        // 3. For each conflict, try to resolve
        for (path, variations) in conflicts {
            match self.resolver_registry.find_unique_resolver(&path) {
                Some(resolver) => {
                    // Exactly 1 resolver matches → call it
                    let resolved = self.call_resolver(resolver, &variations).await?;
                    result.add_file(PathBuf::from(resolved.path), resolved.content.into_bytes());
                }
                None => {
                    // 0 or >1 resolvers match → fall back to LWW
                    let last = variations.into_iter().last().unwrap();
                    result.add_file(path, last);
                }
            }
        }

        Ok(result)
    }
}
```

5. Composition Operator Integration

In operator.rs, build unique resolver list and inject into layerer.

Flow Summary

```
CompositionOperator.execute_composition()
     │
     ├── 1. Execute templates → Vec<VirtualFileSystem>
     │
     ├── 2. Collect UNIQUE resolvers from all templates
     │       (same resolver + different config = different unique resolver)
     │
     ├── 3. ResolverAwareLayerer.layer_merge(vfs_list)
     │       │
     │       ├── Group files by path
     │       │
     │       ├── Non-conflicts (1 variation):
     │       │       └── Add to result directly
     │       │
     │       └── Conflicts (2+ variations):
     │               │
     │               ├── find_unique_resolver(path)?
     │               │       │
     │               │       ├── Exactly 1 match → call resolver
     │               │       ├── 0 matches → LWW fallback
     │               │       └── >1 matches → LWW fallback (ambiguous)
     │               │
     │               └── Add resolved/LWW content to result
     │
     └── 4. Return merged VFS
```

Checklist

- Create cyancoordinator/src/resolver/mod.rs
- Create cyancoordinator/src/resolver/models.rs with ResolverInstance, ResolverInput, ResolverOutput
- Create cyancoordinator/src/resolver/client.rs with ResolverClient trait and HttpResolverClient
- Create cyancoordinator/src/resolver/registry.rs with ResolverRegistry and find_unique_resolver()
- Create ResolverAwareLayerer in layerer.rs
- Implement conflict detection: group_files_by_conflict()
- Implement resolver lookup: exactly 1 match → use, else LWW
- Update CompositionOperator to collect unique resolvers and inject into layerer
- Add configuration for boron URL
- Add unit tests
- Add integration tests

## Comments

(no comments)

---

# Parent: CU-86ewr9nen (task)

- **Title**: Resolver system
- **Status**: todo
- **URL**: https://app.clickup.com/t/86ewr9nen

## Description

Overview
This spec defines a new artifact type called Resolver for the Sulfone platform. Resolvers solve the problem of file conflicts when multiple templates in a composition need to modify the same file.

Problem Statement

Current Behavior: When templates A, B, C, and D all produce the same file (e.g., package.json), the VFS layerer uses "last-wins" semantics.

Desired Behavior: Templates declare a resolver for files they produce. When conflicts occur, the resolver merges all versions intelligently.

Architecture

Port Assignments:
| Artifact Type | Port | Purpose |
|--------------|------|---------|
| Template | 5550 | Interactive Q&A |
| Processor | 5551 | File transformation |
| Plugin | 5552 | Post-processing hooks |
| Resolver | 5553 | Conflict resolution |

Component Responsibilities:
| Component | Responsibility | Sub-plan |
|-----------|---------------|---------|
| Helium | Resolver SDK (Node, Python, .NET) - port 5553 | helium.md |
| Boron | Route to resolvers, start resolver containers | boron.md |
| Zinc | Store resolver artifacts, versions, API endpoints | zinc.md |
| Argon | Display resolvers in UI | argon.md |
| Iridium | Invoke resolvers during VFS merge phase | iridium.md |

Resolver Configuration

Templates declare resolvers in cyan.yaml:

```yaml
resolvers:
  - resolver: 'atomi/json-merger:1'
    config:
      strategy: 'deep-merge'
      array_strategy: 'append'
    files:
      - 'package.json'
      - '**/tsconfig.json'
```

Resolver API (Port 5553)

Endpoint: POST /api/resolve

VFS Merge Algorithm:

1. Resolve dependencies (post-order traversal)
2. Execute each template → VFS (with resolver configs)
3. Layer all VFS with resolver resolution
4. 3-way merge with local files
5. Write to disk

Mathematical Properties: Resolvers SHOULD be commutative and associative.

Migration Path:

- Phase 1: Infrastructure (Helium, Boron, Zinc)
- Phase 2: Registry (Zinc API endpoints, Iridium registry client)
- Phase 3: Execution (Iridium VFS layerer, resolver client)
- Phase 4: UI (Argon resolver pages)

Summary:
| Component | Changes Required |
|-----------|----------------|
| Helium | New resolver SDK (port 5553) |
| Boron | Resolver container management, proxy routing |
| Zinc | Resolver/ResolverVersion models, API endpoints |
| Argon | Resolver pages, navigation, search |
| Iridium | VFS layerer with resolver support |
