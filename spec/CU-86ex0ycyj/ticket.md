# Ticket: CU-86ex0ycyj

- **Type**: task
- **Status**: todo
- **URL**: https://app.clickup.com/t/86ex0ycyj
- **Parent**: none

## Description

## Overview

End-to-end testing that serves as a regression gate for preset answers and post-creation commands features. All feature work depends on passing this E2E suite — it defines what "done" looks like and ensures nothing breaks existing functionality as changes are introduced.

Write E2E tests first (test-driven approach). Features are then implemented to pass the suite.

## Test Scenarios

### Preset Answers

- Template with sub-template dependencies + preset answers in cyan.yaml publishes to Zinc correctly
- During template execution, preset answers are injected and sub-templates skip prompting
- Unsupplied answers still prompt normally
- Preset answers cascade through nested dependency trees

### Post-Creation Commands

- Template with post-creation commands in cyan.yaml publishes to Zinc correctly
- After template execution, Iridium runs declared commands on client side in order
- Execution failures are reported clearly
- Commands that depend on preset answers work correctly (integration)

## Acceptance Criteria

- Full E2E test suite covering both features
- Tests run in CI
- Both features verified working together (preset answers + post-creation commands)
- Existing template functionality remains unbroken (regression gate)

## Comments

No comments.
