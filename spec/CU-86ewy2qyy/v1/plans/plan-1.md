# Plan 1: Environment Variable Substitution for Build and Dev Configs

## Goal

Add a reusable `substitute_env_vars` function in `cyanregistry` and integrate it into the build config and dev config reading pipelines so that all string fields are expanded before validation.

## Files to Modify

| File                                                     | Change                                                         |
| -------------------------------------------------------- | -------------------------------------------------------------- |
| `cyanregistry/src/cli/env_subst.rs`                      | **NEW** — standalone env substitution module                   |
| `cyanregistry/src/cli/mod.rs`                            | Add `pub mod env_subst;`                                       |
| `cyanregistry/src/cli/mapper.rs`                         | Call substitution in `read_build_config` and `read_dev_config` |
| `cyanregistry/src/cli/models/build_config.rs`            | Add `substitute_env` method on `BuildConfig`                   |
| `cyanregistry/src/cli/models/dev_config.rs`              | Add `substitute_env` method on `DevConfig`                     |
| `docs/developer/features/07-environment-substitution.md` | **NEW** — feature documentation                                |

## Approach

### 1. Core Substitution Function (`env_subst.rs`)

Create a new module `cyanregistry/src/cli/env_subst.rs` with:

- A `substitute_env_vars(input: &str) -> Result<String, EnvSubstError>` function
- Parse `${VAR}` and `${VAR:-default}` patterns using simple character-by-character parsing (no regex crate needed — use a state machine or `find`/`split` approach)
- Look up each variable via `std::env::var`
- Error on missing/empty vars without defaults
- Return input unchanged if no `${` patterns found
- A custom `EnvSubstError` type with the variable name for clear error messages

### 2. Config-Level Substitution Methods

Add a `substitute_env` method to both config structs that applies `substitute_env_vars` to every string field:

**`BuildConfig::substitute_env()`** — walks `registry`, `platforms[]`, and each `ImageConfig`'s `image`, `dockerfile`, `context` fields.

**`DevConfig::substitute_env()`** — walks `template_url` and `blob_path`.

These methods return `Result<Self, EnvSubstError>` producing a new config with all fields substituted.

### 3. Integration into Reading Pipeline

In `mapper.rs`:

- `read_build_config`: after deserializing `BuildFileConfig` and extracting `BuildConfig`, call `.substitute_env()` before passing to `build_config_mapper` for validation.
- `read_dev_config`: after deserializing `DevConfig`, call `.substitute_env()` before the existing trim/validation logic.

### 4. Documentation

Create `docs/developer/features/07-environment-substitution.md` covering:

- Feature overview and motivation
- Supported syntax (`${VAR}`, `${VAR:-default}`)
- Which config sections support it (build, dev)
- Examples of usage in `cyan.yaml`
- Error behavior for missing variables

## Testing Strategy

### Unit Tests in `env_subst.rs`

- Basic substitution: `${HOME}` resolves
- Default value: `${MISSING:-fallback}` returns "fallback"
- Missing var without default: returns error with var name
- Empty var without default: returns error
- Empty var with default: uses default
- Multiple vars in one string: `${A}/${B}` both expand
- No vars: passthrough unchanged
- Partial pattern: `${` without closing `}` — treat as literal or error (suggest literal passthrough)

### Integration Tests in `mapper.rs`

- `read_build_config` with env vars in registry and image fields
- `read_dev_config` with env vars in template_url
- Verify validation still catches empty results after substitution (e.g., `${EMPTY_VAR}` resolves to empty, fails validation)

## Edge Cases

- `${VAR:-}` — empty default, resolves to empty string (may fail downstream validation, which is correct)
- Field with multiple vars: `${REGISTRY}/${IMAGE}:${TAG}` — all three expand
- `Option<String>` fields that are `None` — skip substitution (nothing to substitute)

## Implementation Checklist

- [ ] Create `cyanregistry/src/cli/env_subst.rs` with `substitute_env_vars` function
- [ ] Add `EnvSubstError` error type with variable name context
- [ ] Add `pub mod env_subst` to `cyanregistry/src/cli/mod.rs`
- [ ] Add `substitute_env()` method to `BuildConfig`
- [ ] Add `substitute_env()` method to `DevConfig`
- [ ] Integrate into `read_build_config` (after deser, before validation)
- [ ] Integrate into `read_dev_config` (after deser, before validation)
- [ ] Unit tests for `substitute_env_vars`
- [ ] Integration tests for build config with env vars
- [ ] Integration tests for dev config with env vars
- [ ] Create `docs/developer/features/07-environment-substitution.md`
- [ ] Run `pls lint` and ensure all checks pass
