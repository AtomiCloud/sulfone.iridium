# Task Specification: Batch VFS Merges Before Write (CU-86ewra5kn)

## Source

- Ticket: CU-86ewra5kn
- System: ClickUp
- URL: https://app.clickup.com/t/86ewra5kn

## Objective

Refactor the upgrade and create flows to layer all VFS outputs in memory before writing to disk. This fixes a bug where unresolved conflict markers from one template's merge are fed into subsequent merges, and enables future conflict resolution features.

## Acceptance Criteria

- [ ] `pls update` processes all templates, **layering** VFS outputs in memory (LWW), then ONE 3-way merge with local files, then write once
- [ ] `pls create` on existing project re-runs all existing templates (with stored answers) + new template, layers all VFS outputs, then ONE 3-way merge with local files, then write once
- [ ] Templates are processed in order by time/history ID for last-write-wins (LWW) semantics
- [ ] No intermediate disk writes during batch processing
- [ ] New tests cover the batch layering behavior (no existing tests currently)

## Definition of Done

- [ ] All acceptance criteria met
- [ ] `pls lint` passes
- [ ] `pls build` passes
- [ ] Tests pass - note: writing tests from scratch as part of this task
- [ ] Ticket ID included in commit message
- [ ] PR description references ticket

## Out of Scope

- Conflict resolution UI/UX (future ticket)
- New CLI commands or flags
- Changes to `.cyan_state.yaml` format
- Dependency resolution changes
- Single-template upgrade behavior (already works)

## Technical Constraints

- Rust codebase using `git2` for 3-way merges
- Must maintain backwards compatibility with existing projects
- VFS layering and composition must still work correctly
- Must handle the case where cyan_state.yaml has no templates (fresh project)

## Context

### Current Problem

The current flow for `pls update`:

```
For each template in cyan_state.yaml:
  1. VFS merge (prev + local + current)
  2. WRITE to disk
  3. Next template uses WRITTEN files as "local"
```

**Bug:** If template A's merge produces conflict markers (`<<<<<<<`), those markers get written to disk. Then template B's merge uses those conflict markers as "local" input, corrupting the merge.

### Desired Flow (Reuse Existing Layerer - Collect => Merge)

The VFS layerer already exists for dependencies within a template. We reuse the same pattern at a higher level: **collect all VFSs, then merge once**.

```
// COLLECT phase
all_prev_vfs = []
all_curr_vfs = []

for each template in cyan_state.yaml (sorted by time/history ID):
  1. Execute previous version:
     - Template execution internally: collect deps → layer_merge → VFS_n_prev
  2. Execute current version:
     - Template execution internally: collect deps → layer_merge → VFS_n_curr
  3. all_prev_vfs.push(VFS_n_prev)
  4. all_curr_vfs.push(VFS_n_curr)

// MERGE phase - ONE shot (same pattern as dependency layering)
master_VFS_prev = layerer.layer_merge(all_prev_vfs)
master_VFS_curr = layerer.layer_merge(all_curr_vfs)

// 3-way merge with local
VFS_local = load_local_files(target_dir)
merged_VFS = vfs.merge(master_VFS_prev, VFS_local, master_VFS_curr)

// Write once
vfs.write_to_disk(target_dir, merged_VFS)
```

**Key insight:** Same **collect => merge** pattern at two levels:

1. **Within template**: collect dep VFSs → layer_merge (existing)
2. **Across templates**: collect template VFSs → layer_merge (new)

### Same Change for `pls create`

When running `pls create <template> .` on an existing project:

```
// COLLECT phase
all_vfs = []

for each existing template in cyan_state.yaml:
  1. Re-run template with stored answers
     - Template execution: collect deps → layer_merge → VFS_n
  2. all_vfs.push(VFS_n)

// New template
run Q&A and execute → VFS_new
all_vfs.push(VFS_new)

// MERGE phase - ONE shot
master_VFS = layerer.layer_merge(all_vfs)

// 3-way merge with local
VFS_local = load_local_files(target_dir)
merged_VFS = vfs.merge(base_VFS, VFS_local, master_VFS)

// Write once
vfs.write_to_disk(target_dir, merged_VFS)
```

## Technical Decisions

| Decision                             | Choice                     | Reasoning                                                     |
| ------------------------------------ | -------------------------- | ------------------------------------------------------------- |
| In-memory accumulation               | Reuse existing VFS layerer | Same mechanism used for dependencies; apply at template level |
| Merge timing                         | ONE 3-way merge at the end | Prevents conflict marker propagation bug                      |
| Cross-template conflict resolution   | Last-write-wins (LWW)      | Simplest approach; conflict resolution is a future ticket     |
| Template ordering                    | Sort by time/history ID    | Ensures deterministic LWW behavior                            |
| Existing template handling in create | Re-run with stored answers | Ensures fresh VFS output for all templates                    |
| Write strategy                       | Single write at end        | Prevents conflict marker propagation bug                      |

## Edge Cases

- **Empty cyan_state.yaml:** Should work as before (just process the new template)
- **Single template:** Should behave identically to current behavior
- **Template with no changes:** Should still be included in batch layering
- **Failed template fetch:** Should abort the batch operation

## Error Handling

- **Template fetch/execution failure:** Abort entire operation, leave project unchanged
- **Merge failure:** Report error, project unchanged (no write happened yet)
- **Write failure:** Report error, project may be in partial state

## Key Files

| Component            | File Path                                                |
| -------------------- | -------------------------------------------------------- |
| Update Orchestrator  | `cyanprint/src/update/orchestrator.rs`                   |
| Template Processor   | `cyanprint/src/update/template_processor.rs`             |
| Upgrade Executor     | `cyanprint/src/update/upgrade_executor.rs`               |
| Composition Operator | `cyancoordinator/src/operations/composition/operator.rs` |
| VFS Core             | `cyancoordinator/src/fs/vfs.rs`                          |
| VFS Merger           | `cyancoordinator/src/fs/merger.rs`                       |
| VFS Layerer          | `cyancoordinator/src/operations/composition/layerer.rs`  |
| Run Command (create) | `cyanprint/src/run.rs`                                   |
| CyanState Model      | `cyancoordinator/src/state/models.rs`                    |
