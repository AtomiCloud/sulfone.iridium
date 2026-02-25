# Task Specification: Fix error in GitHub Actions run 22318550501/job/64570251380 for sulfone.iridium (86ewpvy6)

## Source

- Ticket: 86ewpvy6
- System: ClickUp
- URL: https://app.clickup.com/t/86ewpvy6

## Objective

Fix a Rust compilation error in the cyanprint crate that caused the GitHub Actions pre-commit Clippy hook to fail. The error was a type mismatch where `exposed_ports` field expected `Vec<String>` but the code was creating tuples and trying to collect them into an inferred type.

## Acceptance Criteria

- [x] Fix the type mismatch error in `cyanprint/src/coord.rs:177`
- [x] Code compiles successfully with `cargo build`
- [x] All pre-commit hooks pass (Clippy, etc.)
- [x] CI workflow passes after the fix

## Definition of Done

- [x] All acceptance criteria met
- [x] Tests pass (validated by CI workflow success)
- [x] No lint/type errors
- [x] Ticket ID included in commit message

## Out of Scope

- Any changes to other files or functionality
- Changes to Docker configuration or networking logic

## Technical Constraints

- Rust project using the bollard crate for Docker API
- Must maintain compatibility with existing container creation logic

## Context

The error occurred during a tag build (v2.4.1) in the pre-commit Clippy hook:

```text
error[E0277]: a value of type `std::vec::Vec<std::string::String>` cannot be built from an iterator over elements of type `(std::string::String, std::collections::HashMap<_, _>)`
   --> cyanprint/src/coord.rs:180:26
    |
180 |                         .collect(),
    |                          ^^^^^^^ value of type `Vec<String>` cannot be built from iterator of `(String, HashMap<_, _>)`
```

The fix was to change from creating tuples to using `Vec<String>` directly:

```rust
// Before (incorrect - creating tuples when Vec<String> is expected)
exposed_ports: Some(
    vec![("9000/tcp".to_string(), HashMap::new())]
        .into_iter()
        .collect(),
),

// After (correct - using Vec<String> directly)
exposed_ports: Some(vec!["9000/tcp".to_string()]),
```

## Root Cause

The `exposed_ports` field in `bollard::models::ContainerCreateBody` expects `Option<Vec<String>>`, but the original code was creating tuples `(String, HashMap)` and trying to collect them, which caused a type mismatch error.
