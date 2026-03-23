# Plan 1: Fix port allocation race condition

## Files changed

- `cyanprint/src/port.rs` (rewrite)
- `cyanprint/Cargo.toml` (dependency addition)
- `cyanprint/src/try_cmd.rs`
- `cyanprint/src/test_cmd/template.rs`
- `cyanprint/src/test_cmd/init.rs`
- `cyanprint/src/test_cmd/container.rs`

## What

Rewrite `port.rs` with bind-and-hold pattern, then migrate all 4 call sites to use the new API with retry logic.

## Steps

### Part A: Rewrite port.rs

1. Add `rand` dependency to `cyanprint/Cargo.toml` if not already present
2. Define 5 port range constants as `pub const` values:
   - `TEMPLATE_TRY: u16 = 49152`, `TEMPLATE_TRY_END: u16 = 49351`
   - `TEMPLATE_TEST: u16 = 49352`, `TEMPLATE_TEST_END: u16 = 49551`
   - `PROCESSOR_TEST: u16 = 49552`, `PROCESSOR_TEST_END: u16 = 49751`
   - `PLUGIN_TEST: u16 = 49752`, `PLUGIN_TEST_END: u16 = 49951`
   - `RESOLVER_TEST: u16 = 49952`, `RESOLVER_TEST_END: u16 = 50151`
3. Implement `PortAllocation` struct with `port: u16` (pub) and `listener: TcpListener` (private)
4. Implement `PortAllocation::release(self) -> u16` — consumes self, drops listener, returns port
5. Implement `allocate_port(range_start, range_end) -> Option<PortAllocation>`:
   - Random selection with `HashSet` tracking, sequential fallback
6. Remove old `find_available_port()` and `is_port_available()`
7. Update/add unit tests

### Part B: Update call sites

#### try_cmd.rs

1. Remove Step 2 port allocation block (~lines 89-98)
2. Add allocation immediately before Step 10 (Docker bind), gated by `!dev_mode`
3. `.release()` right before Docker bind, 3-retry loop

#### test_cmd/template.rs

1. Replace `find_available_port(5600, 5900)` (~line 375) with `allocate_port(TEMPLATE_TEST, TEMPLATE_TEST_END)`
2. `.release()` before `start_template_container()`, 3-retry loop

#### test_cmd/init.rs

1. Replace `find_available_port(5600, 5900)` (~line 1200) with `allocate_port(TEMPLATE_TEST, TEMPLATE_TEST_END)`
2. Same retry pattern as template.rs

#### test_cmd/container.rs

1. Replace old ranges (5500-5599, 5600-5699, 5700-5799) with new constants per artifact type
2. Replace `find_available_port()` (~line 261) with `allocate_port()`
3. `.release()` before Docker `PortBinding`, 3-retry loop
4. `cleanup_image()` only after all retries exhausted

## Implementation Checklist

### port.rs

- [ ] Check if `rand` crate is already in `cyanprint/Cargo.toml` (or workspace deps); add if missing
- [ ] Add `use std::collections::HashSet;` and `use rand::Rng;` imports
- [ ] Define 10 constants (5 ranges x start/end pairs) as `pub const`
- [ ] `PortAllocation::release(self) -> u16` — consumes self, returns `self.port`
- [ ] `allocate_port()`: handle empty range (start > end) -> return `None`
- [ ] `allocate_port()`: random phase — pick up to `min(range_size, 50)` times, skip tried ports
- [ ] `allocate_port()`: sequential fallback — iterate full range, skip tried ports
- [ ] Remove `find_available_port()` and `is_port_available()`
- [ ] Update existing tests to use new API
- [ ] Add test: `test_allocate_port_random_then_sequential_fallback`
- [ ] Add test: `test_release_frees_port`

### try_cmd.rs

- [ ] Remove Step 2 block; verify `allocated_port` not referenced before Step 10
- [ ] New allocation at Step 10 uses `allocate_port(TEMPLATE_TRY, TEMPLATE_TRY_END)`
- [ ] Retry loop (`for _ in 0..3`) around allocate -> release -> Docker bind
- [ ] Error: `"No available port found in range {TEMPLATE_TRY}-{TEMPLATE_TRY_END} after 3 retries"`

### test_cmd/template.rs

- [ ] Replace with `allocate_port(TEMPLATE_TEST, TEMPLATE_TEST_END)`
- [ ] Store `PortAllocation` (not just port number) — listener must stay alive
- [ ] Retry loop, error message update, verify import path

### test_cmd/init.rs

- [ ] Replace with `allocate_port(TEMPLATE_TEST, TEMPLATE_TEST_END)`
- [ ] Same retry pattern as template.rs, verify import path

### test_cmd/container.rs

- [ ] Replace old ranges with `(PROCESSOR_TEST, PROCESSOR_TEST_END)` / `(PLUGIN_TEST, PLUGIN_TEST_END)` / `(RESOLVER_TEST, RESOLVER_TEST_END)`
- [ ] `cleanup_image(&docker, &image_ref)` only after all 3 retries exhausted
- [ ] Verify imports include all 3 range constant pairs

### General

- [ ] Grep for remaining `find_available_port` references — should be zero
- [ ] Grep for old port ranges (`5600`, `5900`, `5500`, `5799`) — verify none remain

## Non-functional Checklist

- [ ] `cargo build -p cyanprint` passes
- [ ] `cargo test -p cyanprint` passes — all existing tests still pass
- [ ] `cargo test -p cyanprint -- port` — new port allocation tests pass
- [ ] `cargo clippy -p cyanprint` passes with no new warnings
- [ ] `cargo fmt --check -p cyanprint` passes
- [ ] `pre-commit run --all` passes
- [ ] New public API (`PortAllocation`, `allocate_port`, range constants) has doc comments
- [ ] Tests written for new behavior
- [ ] No new `#[allow(...)]` attributes added to suppress warnings

## Acceptance criteria

- `allocate_port()` returns a `PortAllocation` that holds the port via `TcpListener`
- `.release()` frees the port (can be rebound by another call)
- All call sites use `allocate_port()` with correct range constants
- Every call site has a 3-retry loop with fresh allocation on each retry
- `.release()` called immediately before Docker bind in every call site
- No stale references to old API or old port ranges
- Build, test, clippy, fmt, pre-commit all pass
