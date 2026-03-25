# Ticket: 86ex0yctp

- **Type**: Task
- **Status**: backlog
- **URL**: https://app.clickup.com/t/86ex0yctp
- **Parent**: 86ewr817w

## Description

Iridium pushes command configs to Zinc during publish. After template execution completes on the client, Iridium executes the declared commands locally in order.

## Comments

No comments available via CLI.

---

# Parent: 86ewr817w (Task)

- **Title**: Post-creation commands (on client side)
- **Status**: in progress
- **URL**: https://app.clickup.com/t/86ewr817w

## Description

## Overview

Templates should be able to declare commands that execute on the client side after template generation completes (e.g., `bun install`, `git init`, setup scripts). Defined in `cyan.yaml` as part of the template config.

## Changes

### [Zn] Store post-creation command configs in registry

- Extend Zinc registry models/API to store post-creation command configs alongside template definition
- Commands are declared per-template in cyan.yaml and pushed to Zinc during publish

### [Ir] Push command configs + execute on template completion

- Iridium pushes the command configs to Zinc during template publish
- After template execution finishes on the client, Iridium executes the declared commands in order
- Commands run locally on the user's machine (not server-side)

## Acceptance Criteria

- cyan.yaml supports declaring post-creation commands
- Commands are stored in Zinc registry and retrievable during template execution
- Iridium executes commands on client side after template files are written
- Commands run in declared order
- Execution failures are reported clearly to the user
