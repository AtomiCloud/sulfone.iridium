# Task Spec: Environment Variable Substitution for Build and Dev Configs

**Ticket:** CU-86ewy2qyy
**Title:** Allow environment substitution for build configs in Iridium

## Summary

Add shell-style environment variable substitution to all string fields in both `build` and `dev` config sections of `cyan.yaml`. This enables dynamic configuration based on the runtime environment (e.g., different registries per CI environment, different dev server URLs per developer).

## Requirements

### Syntax

Support standard shell-style environment variable substitution:

| Pattern           | Behavior                                                      |
| ----------------- | ------------------------------------------------------------- |
| `${VAR}`          | Replace with value of `VAR`. Error if unset or empty.         |
| `${VAR:-default}` | Replace with value of `VAR`. Use `default` if unset or empty. |

Literal `${` sequences that should NOT be substituted are not in scope (no escape mechanism needed unless you see a clear reason to add one).

### Scope

Environment substitution applies to **all string fields** in:

**Build config (`BuildConfig`):**

- `registry` (Option<String>)
- `platforms[]` (Option<Vec<String>>)
- `images.*.image` (Option<String>)
- `images.*.dockerfile` (String)
- `images.*.context` (String)

**Dev config (`DevConfig`):**

- `template_url` (String)
- `blob_path` (String)

### Processing Order

1. YAML deserialization (serde) — raw strings with `${VAR}` placeholders
2. **Environment variable substitution** — expand all `${...}` patterns
3. Validation (existing `build_config_mapper` / `read_dev_config` validation logic)

Substitution happens **before** validation, so a config like `registry: "${REGISTRY}"` with `REGISTRY=ghcr.io/atomicloud` should pass the "registry is required and non-empty" check.

### Error Handling

- **Missing env var without default:** Return a clear error message indicating which variable is missing and which field it was in. Example: `"Environment variable 'REGISTRY' is not set (referenced in build config)"`
- **Empty env var without default:** Same treatment as missing — error.
- **Missing env var with default:** Use the default value silently.

### Backward Compatibility

- Configs without any `${...}` patterns must continue to work exactly as before.
- No changes to the `BuildConfig` or `DevConfig` struct shapes — substitution is a string-processing step, not a model change.

## Acceptance Criteria

1. `${VAR}` in any build/dev config string field is replaced with the env var value
2. `${VAR:-default}` falls back to `default` when `VAR` is unset or empty
3. Missing env vars without defaults produce a clear error before validation runs
4. Existing configs without env vars work identically (no regressions)
5. Unit tests cover: basic substitution, default values, missing vars error, empty vars error, no-op on plain strings, multiple vars in one field
6. Developer documentation added at `docs/developer/features/07-environment-substitution.md`

## Edge Cases

- **Multiple vars in one string:** `${REGISTRY}/${IMAGE_NAME}` — both should be expanded
- **Nested defaults:** `${VAR:-${OTHER}}` — NOT required, can error or treat literally
- **Empty default:** `${VAR:-}` — should resolve to empty string (which may then fail validation)
- **No env vars in string:** Pass through unchanged

## Constraints

- The substitution utility should be a standalone function (e.g., `substitute_env_vars(input: &str) -> Result<String, Error>`) so it can be reused and tested independently
- Do not add external crate dependencies for this — regex from std or simple parsing is sufficient
- Must pass `pls lint` (clippy with `-D warnings`, formatting, etc.)

## Out of Scope

- Env substitution in template configs, processor configs, plugin configs, resolver configs (only build + dev)
- Escape mechanism for literal `${` sequences
- Recursive/nested variable references
- File-based env loading (`.env` files)
