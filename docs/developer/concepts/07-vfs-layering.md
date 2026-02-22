# VFS Layering

**What**: VFS layering merges multiple virtual file systems using overlay semantics (later files overwrite earlier ones).

**Why**: Combines outputs from multiple templates in a composition deterministically.

**Key Files**:

- `cyancoordinator/src/operations/composition/layerer.rs` → `VfsLayerer`
- `cyancoordinator/src/fs/vfs.rs` → `VirtualFileSystem`

## Overview

The Virtual File System (VFS) is an in-memory representation of files. When multiple templates execute in a composition, each produces a VFS. VFS layering combines these into a single VFS.

## Layering Semantics

```mermaid
flowchart TD
    V1[VFS 1<br/>a.txt, b.txt] --> L[Layered VFS]
    V2[VFS 2<br/>b.txt, c.txt] --> L
    V3[VFS 3<br/>c.txt, d.txt] --> L

    L --> LF[Final<br/>a.txt from V1<br/>b.txt from V2<br/>c.txt from V3<br/>d.txt from V3]

    style V1 fill:#90EE90
    style V2 fill:#87CEEB
    style V3 fill:#FFD700
```

**Rule**: Later templates override earlier templates for the same file path.

**Key File**: `cyancoordinator/src/operations/composition/layerer.rs`

## Virtual File System

```rust
pub struct VirtualFileSystem {
    pub(crate) files: HashMap<PathBuf, Vec<u8>>,
}
```

**Key File**: `cyancoordinator/src/fs/vfs.rs`

## Layering Process

1. Create empty VFS
2. For each template's VFS in execution order:
   - Copy all files to layered VFS
   - Overwrite existing paths
3. Return layered VFS

**Key File**: `cyancoordinator/src/operations/composition/layerer.rs`

## Example

Given three templates with outputs:

| Template  | Files                              |
| --------- | ---------------------------------- |
| T1 (base) | `config/default.yaml`, `README.md` |
| T2 (web)  | `config/default.yaml`, `server.rs` |
| T3 (api)  | `routes.rs`                        |

Layered result:

- `config/default.yaml` - from T3 (overwrites T2, which overwrote T1)
- `README.md` - from T1
- `server.rs` - from T2
- `routes.rs` - from T3

## Use in Composition

VFS layering is the final step after all templates execute:

```mermaid
sequenceDiagram
    participant Comp as CompositionOperator
    participant T1 as Template 1
    participant T2 as Template 2
    participant Layer as VfsLayerer

    Comp->>T1: Execute
    T1-->>Comp: VFS 1
    Comp->>T2: Execute
    T2-->>Comp: VFS 2
    Comp->>Layer: layer_merge([VFS 1, VFS 2])
    Layer-->>Comp: Layered VFS
```

**Key File**: `cyancoordinator/src/operations/composition/operator.rs:89-96`

## Related

- [Template Composition](./06-template-composition.md) - Multi-template execution
- [3-Way Merge](../features/02-three-way-merge.md) - Merge with user changes
- [VFS Layering Feature](../features/03-vfs-layering.md) - Feature details
