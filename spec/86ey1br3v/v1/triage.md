# Triage: Record cyanprint-owned files in `.cyan_state.yaml`

## Complexity
moderate

## Repo Set & Dependency Order
Single repo — **`sulfone.iridium`** (the CyanPrint CLI engine).
Path: `/Users/erng/Workspace/atomi/runbook/platforms/sulfone/iridium`. No cross-repo dependencies.

## Assessment
Add two path-only fields to the cyanprint state model: a top-level `managed_files` (sorted union
of every active template's output paths) and a per-template `files` list on each template entry.
Both are recomputed from scratch every run and sourced from each template's individual output VFS
(`curr_vfs_list` in `batch_process`) — not the merged VFS or the working directory. The change
spans the state structs, the per-template path collection during a run, and the state-persistence
path that loads → mutates → writes `.cyan_state.yaml`.

## Things to Check
- `cyancoordinator/src/state/models.rs` — `CyanState` and `TemplateState` structs; confirm field
  names/serde attrs and how `#[serde(flatten)]` on the template map interacts with a new
  `managed_files` sibling field (must not collide with a template literally named `managed_files`).
- `cyanprint/src/run.rs` `batch_process` — confirm `curr_vfs_list` ordering/keying maps cleanly to
  `<user>/<template>` keys; confirm `VirtualFileSystem::get_paths()` exists and returns the paths
  we want (relative? absolute? includes deletions?).
- `cyancoordinator/src/state/services.rs` — `save_template_metadata` / `save_state_file`: confirm
  the load→mutate→write lifecycle and where to inject the recompute so it runs on every save,
  including runs that don't upgrade every template.
- Path normalization — are VFS paths already relative + posix, or do they need normalizing/sorting?
- Inactive/deactivated templates — confirm how `active: false` is set and that they're excluded
  from `managed_files` but may retain (or clear) their own `files`.
- File deletions / cleanup (`run.rs` cleanup step) — does a template output path list include files
  that get deleted during 3-way merge? Confirm `files` reflects template *output*, not post-cleanup.
- Existing tests around state serialization (sample `.cyan_state.yaml`, any snapshot tests) — will
  adding fields break golden files?
- Backward compat — existing state files lack these fields; serde defaults + `skip_serializing_if`
  must round-trip cleanly.

## Open Questions
- For a **deactivated** template (`active: false`), should its `files` list be cleared, frozen at
  last value, or omitted? (managed_files excludes it regardless.) — defer to spec.
- Should `managed_files` include `.cyan_state.yaml` itself or other cyanprint bookkeeping files if a
  template emits them, or are those filtered? — defer to spec.
- Exact path format guarantee (leading `./`? nested dirs as posix `/`?) for stable diffs — defer to
  spec, but lean to relative + posix + sorted.

## Clarifications
- `files` lives **on the template entry** (sibling of `active`/`history`), as a current snapshot
  overwritten each run — confirmed with user in brainstorm.
- **Full union, recomputed every run** (managed_files never partial/stale) — confirmed with user.
- Source = template output, not merged VFS; active templates only — confirmed in brainstorm.

## Risks
Moderate. The state file is a **persisted, user-visible, backward-compatible artifact** shared by
every cyanprint project — schema changes must round-trip with existing files and not corrupt them.
Blast radius is contained to the state model + persistence + one collection point in `batch_process`,
all in a single repo, but serialization correctness and "don't accidentally record user files" are
the real hazards.

## Verification

### Assumptions to Verify
- `VirtualFileSystem::get_paths()` (or equivalent) exists and yields each template's output paths in
  a form we can normalize to relative posix — confirm against `cyancoordinator/src/fs/vfs.rs`.
- `curr_vfs_list` in `batch_process` holds one VFS per active template and can be keyed back to its
  `<user>/<template>` state key — confirm against `cyanprint/src/run.rs`.
- The save path in `services.rs` is the single choke point through which all state writes pass (so a
  recompute there covers every run) — confirm; if not, find the right hook so partial runs still
  produce a complete `managed_files`.
- serde round-trips existing `.cyan_state.yaml` files with the new optional fields without error.

### Access Required
None — all verification is against code already present in the local `sulfone.iridium` checkout.

### Testing Level
moderate

Rationale: a persisted schema change with backward-compat and "don't record the wrong files"
correctness concerns. Unit tests for serialization round-trip + the union/per-template computation,
plus at least one run-level test asserting `managed_files`/`files` populate from template output.

### Validation Matrix
- Automated immediate: serde round-trip test (old file → struct → file); unit test that
  `managed_files` = sorted union of per-template `files`; test that sourcing is template-output (not
  merged VFS / not local files); run existing iridium test suite green.
- Manual immediate: run cyanprint against a sample/fixture template, eyeball the resulting
  `.cyan_state.yaml` for correct `managed_files` + per-template `files`.
- Automated post-release: none.
- Manual post-release: none.
