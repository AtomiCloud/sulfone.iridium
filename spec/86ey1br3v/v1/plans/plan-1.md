---
repo: sulfone.iridium
---

# Plan 1: Record cyanprint-managed files in `.cyan_state.yaml`

## Overview

This single, self-contained slice adds a path-only manifest of cyanprint-managed files to the state
file and is committable on its own: it builds, ships unit tests that exercise the new behavior, and
is independently revertable.

It delivers the whole feature end-to-end:

1. Two new optional, backward-compatible fields on the state model — a top-level `managed_files` and
   a per-template `files`.
2. Collection of each **active** template's own output paths from its individual VFS in
   `batch_process`, **before** the LAYER/MERGE phases consume them — so the manifest is sourced from
   template output, never the merged result or the user's local files.
3. Exclusion of cyanprint's own bookkeeping files and normalization of paths (relative, forward
   slash, sorted, de-duplicated).
4. Full recompute + overwrite of both lists on every run, persisted at the same `save_state_file`
   choke point that already rewrites `file_conflicts`.

Addresses spec goals G1, G2 and requirements FR1–FR8.

## Changes

### 1. `cyancoordinator/src/state/models.rs` — extend the state model

- Add to `CyanState` (currently `templates` (flattened) + `file_conflicts`):
  ```rust
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub managed_files: Vec<String>,
  ```
- Add to `TemplateState` (currently `active` + `history`):
  ```rust
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub files: Vec<String>,
  ```
- `#[serde(default, skip_serializing_if = "Vec::is_empty")]` mirrors the existing `file_conflicts`
  pattern: old state files (no such fields) deserialize fine, and empty lists are omitted on write so
  no noise is added to projects whose templates produce nothing.
- **Caution (flatten collision):** `CyanState.templates` is `#[serde(flatten)]` over a map keyed by
  `<user>/<template>`. The new `managed_files` is a sibling top-level key. Confirm a template can
  never legitimately be named `managed_files` (template keys are `<user>/<name>`, always containing
  `/`, so `managed_files` as a bare key cannot collide). Add a serde round-trip test to lock this in.
- Update any `TemplateState { active, history }` struct literals that will now miss the new field —
  notably `services.rs:77` in `save_template_metadata` (written via the import alias
  `YamlTemplateState { active, history }`), plus any test fixtures — default `files: Vec::new()`.

### 2. `cyancoordinator/src/fs/vfs.rs` — none required

`VirtualFileSystem::get_paths()` already returns `Vec<PathBuf>` of the template's files. Reused as-is.

### 3. `cyanprint/src/run.rs` — collect per-template paths in `batch_process`

- In the `curr_specs` execution loop (`run.rs:86–102`), the per-template output VFS is `vfs` and is
  pushed into `curr_vfs_list` (consumed later at the LAYER phase, `run.rs:125–136`). Collect its
  paths **before** that consumption, keyed by `spec.key()` (which is `"{username}/{template_name}"`,
  `spec.rs:58` — identical to the state-file key built in `services.rs:62`):
  ```rust
  // keyed by "<user>/<template>", value = normalized, filtered, sorted, deduped paths
  let mut managed_by_template: HashMap<String, Vec<String>> = HashMap::new();
  // inside the curr loop, right after `curr_vfs_list.push(vfs);` reads `vfs`:
  managed_by_template.insert(spec.key(), normalize_managed_paths(curr_vfs_list.last().unwrap()));
  ```
  (Collect from `curr_vfs_list` — the **active** set — not `prev_vfs_list`, which is only the 3-way
  merge baseline. This satisfies "active templates only".)
- Add a small free function in `run.rs` (or a `state` helper module):
  ```rust
  fn normalize_managed_paths(vfs: &VirtualFileSystem) -> Vec<String> {
      let mut v: Vec<String> = vfs.get_paths().iter()
          .map(normalize_path)                 // relative, forward-slash, strip leading "./" and "/"
          .filter(|p| !is_cyanprint_internal(p))
          .collect();
      v.sort();
      v.dedup();
      v
  }
  ```
  - `normalize_path`: render the `PathBuf` with `/` separators (`components` joined by `/`, not the
    OS separator), strip a leading `./` and any leading `/`, strip a trailing `/`. VFS paths are
    already stored relative (via `strip_prefix` in the loader/unpacker), so this is normalization,
    not relativization.
  - `is_cyanprint_internal`: excludes cyanprint's own bookkeeping. The set, derived from the
    codebase's literal usages, is the state file `.cyan_state.yaml` (the only cyanprint-internal
    file the loader already special-cases, `fs/loader.rs:25,79`) and the `.cyan_output` default
    output artifact (`commands.rs:205…`). Implement as an exact-name / top-level match against a
    documented `const CYANPRINT_INTERNAL_FILES: &[&str] = &[".cyan_state.yaml", ".cyan_output"];`
    so the set is easy to audit and extend.
- Extend `batch_process`'s return tuple to include `managed_by_template`:
  `(Vec<String>, Vec<FileConflictEntry>, Vec<String>, HashMap<String, Vec<String>>)`
  (update the signature at `run.rs:51`, the final `Ok((...))` at `run.rs:201`, and the call site at
  `run.rs:454`).

### 4. `cyanprint/src/run.rs` — persist in `cyan_run` (the per-run choke point)

- At the existing persistence block (`run.rs:467–471`) — which loads state, sets `file_conflicts`,
  and saves — additionally apply the manifest in the same load→mutate→save:
  ```rust
  // after `cyan_state.file_conflicts = file_conflicts;`
  for (key, ts) in cyan_state.templates.iter_mut() {
      ts.files = managed_by_template.get(key).cloned().unwrap_or_default();
  }
  let mut all: Vec<String> = managed_by_template.values().flatten().cloned().collect();
  all.sort();
  all.dedup();
  cyan_state.managed_files = all;
  // then save_state_file(...)
  ```
  This is the correct hook (not `save_template_metadata`, which only runs for _upgraded_ templates):
  every active template is in `managed_by_template` for this run, so per-template `files` and the
  union `managed_files` are recomputed wholesale and overwritten each run — never partial or stale.
  - A template that is active but absent from `managed_by_template` (shouldn't happen, but defensive)
    gets `files = []`; a deactivated template (`active: false`) is naturally absent from
    `managed_by_template` (only `curr_specs` were collected) so it contributes nothing and its
    `files` is cleared — satisfying "deactivated templates ignored entirely".

### 5. Tests — `cyancoordinator/src/state/` (new `#[cfg(test)]` module) + `run.rs` helper tests

Unit tests colocated with the code, covering FR1–FR8 (see Acceptance Criteria for the exact checks).

### 6. `Taskfile.yaml` — add a `test` route so `pls test` is authoritative

The Taskfile currently defines `build` (`cargo build --release`) and `lint`
(`pre-commit run --all-files`) but **no `test` task**, so `pls test` doesn't resolve. Add one so the
project's authoritative runner (`pls`, which routes Taskfile.yaml targets) can run this plan's unit
tests — validation goes through `pls`, never raw `cargo`:

```yaml
test:
  desc: 'Run unit tests'
  cmds:
    - cargo test
```

(Tooling-only addition that enables the plan's own verification; no product behavior changes.)

## Spec Adherence

- **G1 / FR1** — top-level `managed_files` = sorted union of active templates' outputs (run.rs persist
  block + models.rs field).
- **G1 / FR2** — per-template `files` on each template entry, current snapshot (models.rs field +
  per-run overwrite).
- **G2 / FR3** — sourced from `curr_vfs_list` (per-template output) before LAYER/MERGE, so never the
  user's local files or merge artifacts.
- **G2 / FR4** — only active (`curr_specs`) templates collected; deactivated ones absent → excluded
  and their `files` cleared.
- **G2 / FR5** — recomputed wholesale and overwritten every run at the persist choke point.
- **G2 / FR6** — `is_cyanprint_internal` filters `.cyan_state.yaml`, `.cyan_output`, etc.
- **G1+G2 / FR7** — path-only; `normalize_path` → relative, forward-slash, no leading/trailing slash;
  sorted + de-duplicated.
- **G1 / FR8** — `#[serde(default, skip_serializing_if)]` keeps old state files valid; manifest
  appears on next run with no migration.

## Acceptance Criteria

### Functional Checks

- [ ] **AC1 (FR1)** — Given two active templates whose outputs are `{a.txt, shared.txt}` and
      `{b.txt, shared.txt}`, after a run `managed_files` is exactly `["a.txt", "b.txt", "shared.txt"]`
      (sorted union, de-duplicated).
  - **Evidence (type 1):** `pls test` (and the run-level helper test) →
    test asserting the exact union vector passes; paste the test summary.
- [ ] **AC2 (FR2)** — Each template entry's `files` equals that template's own normalized output
      paths (template A → `["a.txt","shared.txt"]`, template B → `["b.txt","shared.txt"]`).
  - **Evidence (type 1):** `pls test` → per-template `files` assertion
    passes (paste summary).
- [ ] **AC3 (FR3)** — A file present locally in the target dir but produced by NO template does not
      appear in `managed_files` or any `files`; manifest derives only from template output.
  - **Evidence (type 2):** reviewer inspects the collection point in `run.rs` `batch_process` — paths
    come from `curr_vfs_list` entries (template output), gathered before `load_local_files`
    (`run.rs:141`) and `merge` (`run.rs:142`); no local/merged VFS feeds the manifest.
- [ ] **AC4 (FR4)** — A deactivated template (`active: false`, not in `curr_specs`) contributes
      nothing to `managed_files` and has its `files` cleared.
  - **Evidence (type 1):** `pls test` → test with a deactivated template
    asserts empty contribution + cleared `files` (paste summary).
- [ ] **AC5 (FR5)** — Re-running with an unchanged template footprint yields byte-identical lists;
      re-running after a template drops a file removes it from both lists (no stale entries).
  - **Evidence (type 1):** `pls test` → idempotency + footprint-change
    test passes (paste summary).
- [ ] **AC6 (FR6)** — Template output containing `.cyan_state.yaml` / `.cyan_output` is excluded from
      both lists; ordinary project files are kept.
  - **Evidence (type 1):** `pls test` → `is_cyanprint_internal` /
    `normalize_managed_paths` unit test passes (paste summary).
- [ ] **AC7 (FR7)** — Paths are relative, forward-slash, no leading `./` or `/`, no trailing slash,
      sorted and de-duplicated; a backslash/`./`-prefixed input normalizes to the canonical form.
  - **Evidence (type 1):** `pls test` → `normalize_path` table test passes
    (paste summary).
- [ ] **AC8 (FR8)** — A pre-existing `.cyan_state.yaml` WITHOUT the new fields deserializes without
      error and round-trips; serializing a state with empty lists omits `managed_files`/`files`.
  - **Evidence (type 1):** `pls test` → serde backward-compat round-trip
    test passes (paste summary).

### Non-Functional Checks

- [ ] **NFC1** — Workspace builds.
  - **Evidence (type 1):** `pls build` (routes to `cargo build --release`) → exit 0 (paste tail).
- [ ] **NFC2** — Full unit-test suite green (no regressions in existing state/composition tests).
  - **Evidence (type 1):** `pls test` (the new Taskfile route → `cargo test`, whole workspace) → all
    pass (paste summary; the new `state::` manifest tests appear green).
- [ ] **NFC3** — Lint/format clean using the repo's pinned toolchain (NOT bare `cargo clippy`, which
      yields false lints here).
  - **Evidence (type 1):** `pls lint` (routes to `pre-commit run --all-files`, nix-pinned clippy +
    fmt) → all hooks pass (paste tail).

## Validation Approach

- **Immediate automated (the bar for this slice):** unit tests colocated in `cyancoordinator`
  covering the union, per-template snapshot, output-sourcing, active-only, recompute/idempotency,
  bookkeeping exclusion, normalization, and serde backward-compat; plus `pls build` and `pls lint`.
  All validation runs through `pls` (Taskfile routes), never raw `cargo`. The dev loop runs each AC's
  `pls` command and captures its output as evidence.
- **Manual immediate (optional sanity):** run `pls run -- <template>` (routes to `cargo run`) against
  a fixture template in a temp dir and eyeball the resulting `.cyan_state.yaml` for correct
  `managed_files` + per-template `files`. Not required to gate the commit — the unit tests are
  authoritative.
- **e2e:** the repo's `pls e2e` suite is infra-heavy (registry/coordinator) and is **not run** for
  this slice — unit coverage via `pls test` is the gating evidence.
- **Post-release:** none.
