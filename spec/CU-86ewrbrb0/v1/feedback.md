# Feedback for v2

## Commit Reference

c3d5f9000332c31a955a425d783e4ce976e4bddf

## Issues to Fix

### 1. `resolver_ref_config` Convention

`resolver_ref_config` doesn't follow the existing convention. Most of this should be inlined into `template_config` since it's technically just under template.

### 2. `ResolverRefReq` is Broken

The `ResolverRefReq` struct has issues that need to be addressed.

### 3. `config` Should Not Be Nullable

If there's no config, it should be `{}` (empty object), not null.

## Questions/Clarifications (Resolved)

### Layering Implementation Structs

The following structs were reviewed:

- `ResolverInstanceInfo` - stores full resolver config for persisted state
- `TemplateResolverInfo` - lightweight runtime tracking for layering
- `TemplateVariationInfo` - fallback tracking for no-resolver case

**Decision:** These are acceptable as-is for now. Each serves a different lifecycle purpose.
