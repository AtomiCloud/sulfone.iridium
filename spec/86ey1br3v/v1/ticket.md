# Record cyanprint-owned files in .cyan_state.yaml

**ID:** 86ey1br3v
**Status:** backlog
**Type:** task
**List:** Engineering
**URL:** https://app.clickup.com/t/86ey1br3v
**Created:** 2026-06-23
**Updated:** 2026-06-23

**Hierarchy:** No parent task and no subtasks (standalone task).

## Description

Record cyanprint-owned files in .cyan_state.yaml

Problem

.cyan_state.yaml records each template's answers and version history, but never records
which files cyanprint produces. After a run there is no machine- or human-readable manifest
of cyanprint-managed paths, so neither users nor tooling can tell what is generated vs
hand-written.

Desired outcome

After any cyanprint run, .cyan_state.yaml contains:

A top-level managed_files list — the sorted union of every active template's output paths.
A per-template files list on each template entry (sibling of active/history) — that
template's own output paths.

Both are path-only and recomputed in full every run (overwritten, never appended).

Constraints

Paths sourced from template output (each template's own VFS in batch_process, before
layering / 3-way merge) — not the merged VFS and not the working directory. Excludes
user files and post-merge artifacts.
Only active templates contribute to managed_files.
Paths sorted, relative, forward-slash — deterministic YAML diffs.
Backward-compatible with existing state files (serde defaults; empty lists omitted).

Out of scope

No per-file content/hashes/metadata (path-only).
No enforcement/protection of managed files (record only).
No change to the write-to-disk or merge algorithm.

Touch points (engine: sulfone.iridium)

cyancoordinator/src/state/models.rs — add managed_files to CyanState, files to TemplateState.
cyanprint/src/run.rs batch_process — collect per-template paths from curr_vfs_list.
cyancoordinator/src/state/services.rs — recompute + persist on save.
