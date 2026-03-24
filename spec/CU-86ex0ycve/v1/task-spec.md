# Task Spec: CU-86ex0ycve — Push preset configs + inject into dependency tree resolution

## Context

Templates can depend on other templates (e.g., `atomi/cyan` depends on `atomi/workspace`). When a parent template already knows the answers to its child's questions (e.g., `platform=ketone`), it should be able to pre-declare those answers so users aren't re-prompted.

**Parent ticket**: 86ex0ybx9 — Preset answers for sub-templates
**Sibling ticket**: [Zn] Store sub-template preset answer configs in registry (Zinc-side changes)

## Scope

Iridium-side only:

1. Parse preset answer declarations from `cyan.yaml`
2. Include `presetAnswers` in template push requests to Zinc
3. Handle `presetAnswers` in Zinc response models
4. Inject preset answers into sub-template execution during dependency tree resolution

Zinc is assumed to store and return `presetAnswers` (sibling ticket handles Zinc backend).

## Current State

- `cyan.yaml` `templates` field: `Vec<String>` — plain references like `"atomi/workspace:1"`
- `CyanTemplateRef` domain model: `{username, name, version}` — no preset data
- `TemplateRefReq` HTTP request: `{username, name, version}` — no preset data
- `TemplateVersionRes.templates`: `Vec<TemplateVersionPrincipalRes>` — dependency references carry only `{id, version, ...}`
- `DependencyResolver` returns `Vec<TemplateVersionRes>` — no way to carry per-dependency preset answers
- `CompositionOperator.execute_composition()` passes `shared_answers` to each template execution — no per-template answer injection

## Design

### cyan.yaml format (backward compatible)

```yaml
templates:
  - template: atomi/workspace:1
    preset_answers:
      platform: ketone
      use_docker: true
      features: [a, b, c]
  - atomi/platform:2 # plain string still works (no presets)
```

Supported value types: string, bool, array (of strings).

### Data model chain

```
cyan.yaml (CyanTemplateFileRef)
  → CyanTemplateRef (domain)
    → TemplateRefReq (HTTP request to Zinc)
      → Zinc stores → Zinc returns
        → TemplateVersionTemplateRefRes (HTTP response)
          → ResolvedDependency (resolver output)
            → Answer injection before template execution
```

### Preset answer storage format

Stored as `HashMap<String, serde_json::Value>` throughout the pipeline (cyanregistry layer). Converted to `Answer` enum (cyanprompt layer) only at injection time in the composition operator.

### Injection semantics

Preset answers are merged into the per-template execution call using `or_insert` — user-provided answers take precedence, preset answers fill gaps. Scoped to the specific dependency template; not leaked to siblings or subsequent templates in the composition.

### serde naming

Rust: `preset_answers` | JSON: `presetAnswers` (via `#[serde(rename_all = "camelCase")]`)

## Changes

### 1. cyanregistry — Config parsing

**`cyanregistry/src/cli/models/template_config.rs`**

- Add `CyanTemplateFileRef` enum (untagged): `Simple(String)` | `Extended { template: String, preset_answers: HashMap<String, serde_json::Value> }`
- Change `CyanTemplateFileConfig.templates` from `Vec<String>` to `Vec<CyanTemplateFileRef>`

### 2. cyanregistry — Domain model

**`cyanregistry/src/domain/config/template_config.rs`**

- Add `preset_answers: HashMap<String, serde_json::Value>` to `CyanTemplateRef`

### 3. cyanregistry — CLI mapper

**`cyanregistry/src/cli/mapper.rs`**

- Update `template_reference_mapper` signature to accept `&CyanTemplateFileRef`
- Parse `Simple` variant as before with empty `preset_answers`
- Parse `Extended` variant with preset answers
- Update `template_config_mapper` to use new mapper

### 4. cyanregistry — HTTP request model

**`cyanregistry/src/http/models/template_req.rs`**

- Add `#[serde(rename_all = "camelCase")]` to `TemplateRefReq`
- Add `preset_answers: HashMap<String, serde_json::Value>` field with `#[serde(default)]`

### 5. cyanregistry — HTTP response model

**`cyanregistry/src/http/models/template_res.rs`**

- Add `TemplateVersionTemplateRefRes` struct: `{id, version, created_at, description, properties, preset_answers}` with `#[serde(rename_all = "camelCase")]` and `#[serde(default)]` on `preset_answers`. Matches Zinc's `TemplateVersionTemplateRefResp` shape (TemplateVersionPrincipal fields + presetAnswers).
- Change `TemplateVersionRes.templates` from `Vec<TemplateVersionPrincipalRes>` to `Vec<TemplateVersionTemplateRefRes>`

### 6. cyanregistry — HTTP mapper

**`cyanregistry/src/http/mapper.rs`**

- Update `template_ref_req_mapper` to copy `preset_answers` from `CyanTemplateRef` to `TemplateRefReq`
- Update existing tests, add new tests for preset_answers

### 7. cyancoordinator — Resolver

**`cyancoordinator/src/operations/composition/resolver.rs`**

- Add `ResolvedDependency` struct: `{template: TemplateVersionRes, preset_answers: HashMap<String, Answer>}`
- Change `DependencyResolver::resolve_dependencies` return type to `Vec<ResolvedDependency>`
- Update `DefaultDependencyResolver`: extract `preset_answers` from parent's `TemplateVersionTemplateRefRes` and convert `serde_json::Value` → `Answer`
- Carry preset answers through recursive flattening (parent presets for a dependency)

### 8. cyancoordinator — Composition operator

**`cyancoordinator/src/operations/composition/operator.rs`**

- Update `execute_composition` to accept `&[ResolvedDependency]` instead of `&[TemplateVersionRes]`
- Before each template execution, merge preset answers into a local copy of `shared_answers` using `or_insert`
- Pass merged answers to `execute_template`
- Update `execute_template` and all call sites

### 9. Answer conversion helper

Add a utility function `serde_json_value_to_answer(value: &serde_json::Value) -> Option<Answer>`:

- String → `Answer::String`
- Bool → `Answer::Bool`
- Array of strings → `Answer::StringArray`
- Other types → None (skip/ignore)

### 10. Tests

- Unit: cyan.yaml parsing with both Simple and Extended variants
- Unit: CLI mapper with preset_answers
- Unit: HTTP mapper with preset_answers
- Unit: serde round-trip for TemplateRefReq and TemplateVersionTemplateRefRes
- Unit: Answer conversion helper
- Update existing tests for changed types (template_config_mapper, template_ref_req_mapper, etc.)

## Out of scope

- Zinc backend storage (sibling ticket)
- E2E test updates (may be added separately)
- Validation of preset answer keys against sub-template's actual questions
