# Task Specification: Remove cache shell and attic dependency (CU-86ewpyvh6)

## Source

- Ticket: CU-86ewpyvh6
- System: ClickUp
- URL: https://app.clickup.com/t/86ewpyvh6

## Objective

Remove the cache shell workflow and eliminate the dependency on attic from the project. The cache shell is a GitHub Actions workflow that builds devShells and pushes them to an attic binary cache. This task removes that functionality and the related attic configurations.

## Acceptance Criteria

- [ ] `scripts/cache-shell.sh` is deleted
- [ ] `.github/workflows/⚡reusable-cacheshell.yaml` is deleted
- [ ] `.github/workflows/cache.yaml` no longer references the cacheshell job
- [ ] `scripts/build.sh` no longer contains attic push logic
- [ ] `.github/workflows/⚡reusable-build.yaml` no longer passes `attic-token` secret
- [ ] All workflows continue to function correctly without attic

## Definition of Done

- [ ] All acceptance criteria met
- [ ] CI passes (builds succeed without attic)
- [ ] No lint/format errors
- [ ] Ticket ID included in commit message

## Out of Scope

- Modifying `flake.lock` - atticpkgs comes from atomipkgs registry as a transitive dependency
- Updating atomipkgs registry
- Any changes to actual application code

## Technical Constraints

- Maintain backward compatibility with the build workflow
- The `cache.yaml` workflow should still trigger the `cachebuild` job
- Build artifacts should still be uploaded correctly

## Context

The project currently uses attic (a Nix binary cache server) to cache devShells and build outputs. This task removes:

1. The cache shell workflow that builds and pushes devShells to attic
2. The attic push logic in the build script
3. The attic-token secret references from workflows

The `atticpkgs` in `flake.lock` is a transitive dependency from the atomipkgs registry and cannot be removed locally.

## Files to Modify/Delete

| File                                           | Action                           |
| ---------------------------------------------- | -------------------------------- |
| `scripts/cache-shell.sh`                       | DELETE                           |
| `.github/workflows/⚡reusable-cacheshell.yaml` | DELETE                           |
| `.github/workflows/cache.yaml`                 | MODIFY (remove cacheshell job)   |
| `.github/workflows/⚡reusable-build.yaml`      | MODIFY (remove attic-token)      |
| `scripts/build.sh`                             | MODIFY (remove attic push logic) |
