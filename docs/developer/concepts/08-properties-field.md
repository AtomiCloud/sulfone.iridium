# Properties Field

**What**: The `properties` field determines whether a template is executable (has Docker images) or is a group template (composition only).

**Why**: Controls whether the coordinator executes the template in a container or skips it during composition.

**Key Files**:

- `cyanprint/src/main.rs:56-88` → `push template` calls `push_template()` with Docker images
- `cyanprint/src/main.rs:89-108` → `push group` calls `push_template_without_properties()`
- `cyanregistry/src/http/mapper.rs:76-106` → `template_req_with_properties_mapper()`
- `cyanregistry/src/http/mapper.rs:108-124` → `template_req_without_properties_mapper()`

## How It's Determined

The `properties` field is **NOT** specified in `cyan.yaml`. It's determined by the **CLI push subcommand**:

| CLI Command                     | `properties` Value         | Template Type |
| ------------------------------- | -------------------------- | ------------- |
| `pls push template <images...>` | `Some(TemplateProperties)` | Executable    |
| `pls push group`                | `None`                     | Group         |

## Properties Structure

When present, `properties` contains Docker execution artifacts:

```rust
pub struct TemplatePropertyReq {
    pub blob_docker_reference: String,
    pub blob_docker_tag: String,
    pub template_docker_reference: String,
    pub template_docker_tag: String,
}
```

**Key File**: `cyanregistry/src/http/models/template_req.rs`

## Execution Behavior

| `properties` | Behavior                                                 |
| ------------ | -------------------------------------------------------- |
| `Some(...)`  | Template executes in Docker container, generates files   |
| `None`       | Template skipped during execution, only metadata tracked |

The coordinator checks this at runtime in `operator.rs:44-45`:

```rust
if template.principal.properties.is_none() {
    // Skip - it's a group template
}
```

## Related

- [Template](./01-template.md) - Template types overview
- [Template Group](./02-template-group.md) - Group templates
- [Template Composition](./06-template-composition.md) - How properties affects execution
- [push Command](../surfaces/cli/01-push.md) - CLI subcommands
