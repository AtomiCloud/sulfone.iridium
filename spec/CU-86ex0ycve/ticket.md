# Ticket: CU-86ex0ycve

- **Type**: Task
- **Status**: backlog
- **URL**: https://app.clickup.com/t/86ex0ycve
- **Parent**: 86ex0ybx9

## Description

Iridium pushes preset answer configs to Zinc during template publish. During dependency tree resolution, Iridium injects preset answers into the deterministic state so sub-templates skip prompting for pre-set values.

## Comments

No comments.

---

# Parent: 86ex0ybx9 (Parent)

- **Title**: Preset answers for sub-templates
- **Status**: in progress
- **URL**: https://app.clickup.com/t/86ex0ybx9

## Description

## Overview

Templates should be able to declare preset answers for their sub-template dependencies in `cyan.yaml`. When a template depends on other templates (e.g., `atomi/cyan` depends on `atomi/workspace`), it can pre-fill shared prompts (e.g., `atomi/platform=ketone`) so users aren't re-prompted for values already known.

Defined in `cyan.yaml` as part of the template config — not CLI flags.

## Changes

### [Zn] Store sub-template preset answer configs in registry

- Extend Zinc registry models/API to store preset answer configs alongside dependency declarations
- When a template publishes, its sub-template preset answers are stored in Zinc

### [Ir] Push preset configs + inject into dependency tree resolution

- Iridium pushes the preset answer configs to Zinc during template publish
- During dependency tree resolution, Iridium injects preset answers into the deterministic state so sub-templates skip prompting for pre-set values

## Acceptance Criteria

- cyan.yaml supports declaring preset answers per sub-template dependency
- Preset answers are stored in Zinc registry alongside the template
- Iridium injects preset answers into deterministic state during dependency resolution
- Sub-templates skip prompting for pre-set values (determinism check succeeds)
- Unsupplied answers still trigger normal prompting
