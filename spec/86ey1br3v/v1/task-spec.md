# Spec: Record cyanprint-owned files in `.cyan_state.yaml`

## Summary

cyanprint's state file records *what answers* each template was run with, but never *which files*
cyanprint produces. This change adds a path-only manifest of cyanprint-owned files to the state
file — a whole-project list plus a per-template list — recomputed on every run from each template's
own output. The outcome: anyone (a developer, a script, a future cyanprint feature) can read the
state file and know exactly which files cyanprint manages, separate from the user's own files.

## Background & Context

cyanprint scaffolds and updates projects by running one or more templates and merging their outputs
into the working directory. It persists a state file in each project that, today, captures each
template's active status and a version history of the answers it was run with. That history is
enough to *re-run* a template, but it says nothing about the template's *footprint* — the actual
files it placed in the project.

This is a real gap. Because cyanprint output and hand-written files live side by side in the same
working tree, there is currently no way to tell them apart. Users editing or cleaning up a project
can't see what is safe to touch, and tooling (linters, cleanup scripts, CI, future cyanprint
capabilities) has no authoritative manifest of managed paths to build on. The information needed to
close this gap already exists transiently during a run — each template's output is computed in
memory before everything is merged — it simply is never recorded.

A key subtlety drives the design: cyanprint merges every template's output together with the
project's existing local files to produce the final tree. A trustworthy "what cyanprint manages"
manifest must be derived from the *template outputs themselves*, before that merge — otherwise it
would conflate cyanprint's footprint with the user's own files and with merge artifacts, defeating
the purpose.

## Goals

- **G1** — From the state file alone, a person or tool can see exactly which files cyanprint
  manages in a project, both as a whole (one project-wide list) and broken down per template.
- **G2** — That manifest is trustworthy and current: it reflects cyanprint's *own* output only
  (never the user's files or post-merge artifacts), and is complete and accurate after every run.

## Approach (high level)

Record two path-only lists in the state file:

1. A **project-wide manifest** — the union of the files produced by all currently-active templates.
2. A **per-template manifest** — for each template, the files that template alone produced, stored
   alongside that template's existing entry as a single current snapshot.

Both are sourced from each template's individual output (which cyanprint already computes during a
run, prior to merging outputs together and with the user's local files), and both are recomputed in
full and overwritten on every run. Because a run already re-executes every active template, the
complete picture is always available — so the manifest is rebuilt wholesale rather than patched,
guaranteeing it can never drift into a partial or stale state. The lists carry paths only — no
content, hashes, timestamps, or other metadata — and are emitted in a stable, deterministic order
so the state file produces clean, reviewable diffs run-to-run.

Three decisions shape what goes into the lists and how paths are written:

- **Deactivated templates are ignored entirely.** Only active templates are considered. A template
  that is not active contributes nothing to the project-wide manifest and carries no per-template
  list — it is simply skipped, not frozen or carried forward.
- **cyanprint's own bookkeeping files are excluded.** Files that are cyanprint's own control/metadata
  artifacts — the state file itself (`.cyan_state.yaml`), cyanprint output/bookkeeping files such as
  `.cyan_output`, and similar cyanprint-internal files — never appear in the manifest. The manifest
  describes the *project* files cyanprint produces, not cyanprint's own machinery.
- **Paths are normalized to a stable, portable form:** relative to the project root, using forward
  slashes on every platform, with no leading `./` or `/` and no trailing slash; the lists are sorted
  lexicographically and de-duplicated. This guarantees identical, clean diffs across operating
  systems and across runs.

The change is additive and backward-compatible: existing state files that predate the manifest stay
valid, and gain the new information the next time cyanprint runs.

## Requirements (derived from the Goals)

- **FR1** (→ G1) — The state file contains a project-wide list of every file cyanprint manages,
  comprising the union of all currently-active templates' outputs.
- **FR2** (→ G1) — Each template's entry in the state file carries its own list of the files that
  template produced, reflecting what it currently manages (a current view refreshed each run, not a
  historical accumulation).
- **FR3** (→ G2) — Both lists are derived from template *output* — each template's own produced
  files — and never include the user's pre-existing local files or files that only exist as a
  result of merging.
- **FR4** (→ G2) — Only currently-active templates are considered. A deactivated template is ignored
  entirely: it contributes nothing to the project-wide list and carries no per-template list (its
  list is not retained, frozen, or carried forward).
- **FR5** (→ G2) — Both lists are recomputed from scratch and overwritten on every run, so they are
  always complete and current and never accumulate stale entries.
- **FR6** (→ G2) — cyanprint's own bookkeeping/control files are excluded from both lists — the state
  file itself (`.cyan_state.yaml`), cyanprint output/bookkeeping artifacts (e.g. `.cyan_output`), and
  similar cyanprint-internal files. The manifest lists project files cyanprint produces, not
  cyanprint's own machinery.
- **FR7** (→ G1, → G2) — The lists contain paths only — no file content or per-file metadata. Paths
  are normalized to a stable, portable form: relative to the project root, forward-slash separated on
  every platform, no leading `./` or `/` and no trailing slash, sorted lexicographically and
  de-duplicated — so diffs are identical across operating systems and across runs.
- **FR8** (→ G1) — Existing state files without the manifest remain valid and readable; cyanprint
  populates the new information on the next run without manual migration.

## Non-Goals / Out of Scope

- No per-file content, hashes, timestamps, or ownership metadata — the manifest is path-only.
- No enforcement or protection of managed files (e.g. refusing to overwrite user edits, or
  auto-deleting removed files based on the manifest) — this change *records*, it does not police.
- No change to how cyanprint writes files to disk, resolves conflicts, or merges template outputs.
- No new commands or UI to query the manifest — it is simply present in the state file for now.

## Open Questions & Risks

**Resolved decisions** (previously open, now settled with the stakeholder):

- **Deactivated templates** — ignored entirely (see FR4): excluded from the project-wide list and
  given no per-template list.
- **Bookkeeping files** — cyanprint's own control/metadata files (`.cyan_state.yaml`, `.cyan_output`,
  etc.) are excluded from the manifest (see FR6).
- **Path representation** — normalized to relative, forward-slash, no leading/trailing slash, sorted,
  de-duplicated (see FR7).

**Remaining risks:**

- **Risk — schema/serialization compatibility:** the state file is a persisted, user-visible
  artifact in every cyanprint project. Adding fields must round-trip cleanly with pre-existing
  files and must not corrupt or reorder existing data. This is the primary risk and will be
  verified explicitly in the plan/implementation phase.
- **Risk — sourcing correctness:** the manifest must be taken from template output, not the merged
  result; getting this wrong would silently record the wrong files. The plan phase will confirm the
  exact point in the run where per-template output is available and untainted by the merge.
- **To confirm in the plan — exact bookkeeping set:** the precise list of cyanprint-internal files to
  exclude (FR6) must be pinned down from the codebase, so the exclusion is complete and doesn't
  accidentally drop a legitimate template-produced file or keep a cyanprint-internal one.

## Success Criteria

- After a cyanprint run, the state file shows a project-wide list of managed files that matches the
  union of what the active templates produced.
- After a cyanprint run, each active template's entry shows its own list of produced files.
- Files that exist in the project but were not produced by any template (the user's own files) do
  not appear in either list.
- cyanprint's own bookkeeping files (`.cyan_state.yaml`, `.cyan_output`, and similar) do not appear
  in either list, even though cyanprint creates them.
- A deactivated template contributes nothing — it has no per-template list and adds nothing to the
  project-wide list.
- Recorded paths are relative, forward-slash, sorted and de-duplicated, and are byte-for-byte
  identical whether the run happens on Windows, macOS, or Linux.
- Re-running cyanprint with no changes produces the same lists, in the same order, with no spurious
  diffs; running after a template's footprint changes updates the lists to match, with no leftover
  stale entries.
- A pre-existing state file (created before this change) is read without error and gains the
  manifest on the next run.
