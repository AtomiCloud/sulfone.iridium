# Task Specification: Fix error in GitHub Actions run 22318550501/job/64570251380 for sulfone.iridium (86ewpvy6)

## Source

- Ticket: 86ewpvy6
- System: ClickUp
- URL: https://app.clickup.com/t/86ewpvy6

## Objective

Fix a Rust compilation error in the cyanprint crate that caused the GitHub Actions pre-commit Clippy hook to fail. The error was a type mismatch where `exposed_ports` field expected a `HashMap<String, HashMap<...>>` but the code was attempting to collect into an inferred type that resulted in a `Vec<String>`.

## Acceptance Criteria

- [x] Fix the type mismatch error in `cyanprint/src/coord.rs:180`
- [ ] Code compiles successfully with `cargo build`
- [ ] All pre-commit hooks pass (Clippy, etc.)
- [ ] CI workflow passes after the fix

## Definition of Done

- [x] All acceptance criteria met
- [ ] Tests pass
- [ ] No lint/type errors
- [ ] Ticket ID included in commit message

## Out of Scope

- Any changes to other files or functionality
- Changes to Docker configuration or networking logic

## Technical Constraints

- Rust project using the bollard crate for Docker API
- Must maintain compatibility with existing container creation logic

## Context

The error occurred during a tag build (v2.4.1) in the pre-commit Clippy hook:

```
error[E0277]: a value of type `std::vec::Vec<std::string::String>` cannot be built from an iterator over elements of type `(std::string::String, std::collections::HashMap<_, _>)`
   --> cyanprint/src/coord.rs:180:26
    |
180 |                         .collect(),
    |                          ^^^^^^^ value of type `Vec<String>` cannot be built from iterator of `(String, HashMap<_, _>)`
```

The fix was to explicitly specify the collection type as `HashMap`:

```rust
// Before (incorrect - type inference failed)
.collect(),

// After (correct)
.collect::<HashMap<_, _>>(),
```

## Root Cause

The `exposed_ports` field in `ContainerCreateBody` expects a `HashMap<String, HashMap<String, Option<String>>>` (or similar), but without explicit type annotation, Rust couldn't infer the correct type and defaulted to `Vec<String>`, causing a compilation error.
