# Template

**What**: A template is a packaged unit that generates project files through user prompts.

**Why**: Templates enable developers to quickly scaffold projects with consistent structure and best practices.

**Key Files**:

- `cyanregistry/src/http/models/template_res.rs` → `TemplateVersionRes`
- `cyancoordinator/src/template/executor.rs` → `execute_template()`

## Overview

A template is stored in the registry and contains:

- **Metadata** - Name, version, description
- **Properties** - Execution artifacts (Docker images for executable templates)
- **Dependencies** - List of template references this template depends on

Templates come in two forms:

### Executable Template

Contains execution artifacts (Docker images) and generates files when run. The template runs in an isolated container, prompts the user for answers, and returns an archive of generated files.

**Key indicators**: `properties` field is present in template metadata

### Group Template

A template without execution artifacts that serves as a composition of other templates. Group templates declare dependencies but don't generate files directly.

**Key indicators**: `properties` field is `None` in template metadata

> See [Properties Field](./08-properties-field.md) for how `properties` is determined at push time.

## Template Reference Format

Templates are referenced using the format:

```text
<username>/<template-name>:<version>
```

Example:

```text
atomicloud/starter:1
```

**Key File**: `cyanprint/src/util.rs` → `parse_ref()`

## Template Metadata

Stored in `.cyan_state.yaml` after execution:

```yaml
templates:
  username/template-name:
    history:
      - version: 1
        answers:
          question-id: answer-value
        deterministic_states:
          state-id: state-value
```

**Key File**: `cyancoordinator/src/state/services.rs` → `save_template_metadata()`

## Related

- [Template Group](./02-template-group.md) - Composition of templates
- [Answer Tracking](./03-answer-tracking.md) - Storing template answers
- [Template Composition](./06-template-composition.md) - Multi-template execution
