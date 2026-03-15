# Environment Variable Substitution

**What**: String interpolation of environment variables in `cyan.yaml` configuration files.

**Why**: Enables dynamic configuration without hardcoding values, supporting different environments (dev, CI, prod).

**Key Files**:

- `cyanregistry/src/cli/env_subst.rs` → `substitute_env_vars()`
- `cyanregistry/src/cli/models/build_config.rs` → `BuildConfig::substitute_env()`
- `cyanregistry/src/cli/models/dev_config.rs` → `DevConfig::substitute_env()`
- `cyanregistry/src/cli/mapper.rs` → Integration points

## Overview

Environment variable substitution allows you to reference environment variables in your `cyan.yaml` file using standard shell-like syntax. Substitution occurs during configuration parsing, before validation, ensuring that all validation rules apply to the expanded values.

## Supported Syntax

| Syntax            | Description                                                    |
| ----------------- | -------------------------------------------------------------- |
| `${VAR}`          | Substitutes with the value of `VAR`. Errors if unset or empty. |
| `${VAR:-default}` | Substitutes with `default` if `VAR` is unset or empty.         |

## Supported Configuration Sections

Environment variable substitution is applied to all string fields in:

### Build Configuration (`build:`)

- `registry` - Container registry URL
- `platforms[]` - Target platforms list
- `images.*.image` - Image names
- `images.*.dockerfile` - Dockerfile paths
- `images.*.context` - Build context paths

### Dev Configuration (`dev:`)

- `template_url` - External template server URL
- `blob_path` - Path to blob directory

## Example Usage

### Basic Substitution

```yaml
build:
  registry: ${CONTAINER_REGISTRY}
  images:
    template:
      image: my-template
      dockerfile: Dockerfile
      context: .
```

With `CONTAINER_REGISTRY=ghcr.io/atomicloud`, this becomes:

```yaml
build:
  registry: ghcr.io/atomicloud
  images:
    template:
      image: my-template
      dockerfile: Dockerfile
      context: .
```

### Default Values

```yaml
build:
  registry: ${CONTAINER_REGISTRY:-ghcr.io/default}
  images:
    template:
      image: ${IMAGE_NAME:-my-template}
      dockerfile: Dockerfile
      context: .
```

### Multiple Variables

```yaml
build:
  registry: ${REGISTRY}/${ORG}
  images:
    template:
      image: ${IMAGE}:${TAG:-latest}
      dockerfile: Dockerfile
      context: .
```

### Dev Configuration

```yaml
dev:
  template_url: ${TEMPLATE_SERVER:-http://localhost:8080}
  blob_path: ${BLOB_PATH:-./blob}
```

## Error Behavior

### Missing Variable Without Default

If a referenced environment variable is not set and no default is provided:

```yaml
build:
  registry: ${MISSING_REGISTRY} # Error if MISSING_REGISTRY is not set
```

Error message:

```text
Environment variable 'MISSING_REGISTRY' is not set and no default value was provided
```

### Empty Variable Without Default

If an environment variable is set but empty, and no default is provided:

```yaml
build:
  registry: ${EMPTY_REGISTRY} # Error if EMPTY_REGISTRY="" is set
```

This ensures that empty strings don't silently pass validation.

### Validation After Substitution

All standard validation rules apply after substitution:

```yaml
build:
  registry: ${REGISTRY} # Must not be empty after substitution
  images:
    template:
      image: my-template
      dockerfile: Dockerfile
      context: .
```

If `REGISTRY=""` and using `${REGISTRY:-}` (empty default), substitution succeeds but validation fails with:

```text
build.registry is required
```

Note: Without the `:-` suffix (e.g., `${REGISTRY}`), an empty variable causes an immediate substitution error rather than a validation error.

## Edge Cases

| Case                          | Behavior                                        |
| ----------------------------- | ----------------------------------------------- |
| `${VAR:-}`                    | Uses empty string as default                    |
| `${}`                         | Passed through as literal `${}`                 |
| `${UNCLOSED`                  | Passed through as literal `${UNCLOSED`          |
| `$VAR`                        | Passed through as literal `$VAR` (no braces)    |
| `${VAR:-https://example.com}` | Default values with colons are supported        |
| Nested braces in default      | Limited support, may produce unexpected results |

## Implementation Details

### Substitution Order

1. Parse YAML file into configuration struct
2. Apply `substitute_env()` to walk all string fields
3. For each string field, call `substitute_env_vars()` to expand variables
4. Run validation on the expanded configuration

### Character-by-Character Parsing

The implementation uses a state machine approach rather than regex for several reasons:

- No external dependencies (no regex crate needed)
- Clear error messages with variable name context
- Predictable performance characteristics
- Easy to extend with additional syntax if needed

**Key File**: `cyanregistry/src/cli/env_subst.rs:54-144`

## Related

- [Build Configuration](../surfaces/cli/) - CLI commands using build config
- [Dev Mode](../concepts/) - Development mode using dev config
