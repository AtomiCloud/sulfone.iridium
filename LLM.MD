# LLM Development Guide for CyanPrint (sulfone-iridium)

This guide provides essential information for LLMs working on the CyanPrint project, a Rust-based templating engine.

## Project Overview

**CyanPrint** is a command-line templating engine that allows users to create and use project templates.

- **Platform**: sulfone
- **Service**: iridium
- **Language**: Rust (multi-crate workspace)
- **Crates**: `cyanprint`, `cyanprompt`, `cyanregistry`, `cyancoordinator`

## Essential Commands & Aliases

### Task Runner
- **Primary command**: `pls` (alias for `task`)
- **Available tasks**:
  - `pls build` - Build the binary with `cargo build --release`
  - `pls run` - Run the application with `cargo run`
  - `pls lint` - Run all linting and formatting checks

### Critical Rule: Always Lint Before Completion
**🚨 MANDATORY**: Always run `pls lint` before concluding that work is complete. This runs comprehensive pre-commit hooks including:
- Rust Clippy (with warnings as errors)
- Secrets scanning (Infisical)
- Git commit message linting
- Shell script linting
- Docker linting
- Code formatting

## Development Environment

### Nix-Based Setup
This project uses **Nix flakes** for reproducible development environments:

- **Environment loading**: Automatic via `direnv` (`.envrc`)
- **Flake configuration**: `flake.nix` + `nix/` directory
- **Key Nix files**:
  - `nix/packages.nix` - Available tools and dependencies
  - `nix/pre-commit.nix` - Linting and formatting configuration
  - `nix/env.nix` - Environment variables
  - `nix/shells.nix` - Development shell configuration

### When Working with Commands, Scripts, CI, or Installing Binaries:
1. **Always check the `nix/` folder first** - This defines the working environment
2. **Check `nix/packages.nix`** - See what tools are already available
3. **Use the Nix environment** - Don't install tools outside of Nix unless absolutely necessary
4. **Available tools include**:
   - Rust toolchain (via fenix)
   - Docker, Git, Go, GoReleaser
   - Linting tools: clippy, shellcheck, hadolint, gitlint
   - AtomiCloud tools: pls, sg, atomiutils, toml-cli
   - Security: infisical (secrets scanning)

## Workflow Guidelines

### Always Use Common Workflows in Taskfile
- Check `Taskfile.yaml` for existing tasks before creating new ones
- Use `pls <task>` for all common operations
- Extend the Taskfile for new recurring tasks rather than creating ad-hoc scripts

### Available Helper Scripts
Located in `scripts/` directory:
- `build.sh` - Build operations
- `publish.sh` - Publishing to registries
- `release.sh` - Release management
- `bump.sh` - Version bumping
- `fury.sh` - Fury registry operations
- `pre-commit.sh` - Pre-commit hook management

## Code Quality & Standards

### Pre-commit Hooks (via `pls lint`)
- **Rust**: Clippy with warnings as errors (`-D warnings`)
- **Security**: Infisical secrets scanning (both staged files and commit history)
- **Git**: Commit message linting with gitlint
- **Shell**: Shellcheck for shell scripts
- **Docker**: Hadolint for Dockerfiles
- **Formatting**: Treefmt for consistent code formatting
- **Permissions**: Automatic executable permissions for shell scripts

### Rust-Specific Guidelines
- **Workspace structure**: Multi-crate workspace in `Cargo.toml`
- **Build**: Use `cargo build --release` for production builds
- **Testing**: Run tests with `cargo test`
- **Linting**: Clippy is configured to treat warnings as errors

## CI/CD Information

### GitHub Actions Workflows
Located in `.github/workflows/`:
- `ci.yaml` - Continuous integration
- `release.yaml` - Release automation
- `⚡reusable-*.yaml` - Reusable workflow components
- `🛡️merge-gatekeeper.yml` - Merge protection

### Release Management
- Uses `atomi_release.yaml` for release configuration
- GoReleaser for binary distribution
- Automated changelog generation
- Version bumping scripts available

## File Structure Guidelines

### Key Configuration Files
- `Taskfile.yaml` - Task definitions (use `pls` to run)
- `flake.nix` - Nix environment configuration
- `.envrc` - Direnv configuration for automatic environment loading
- `Cargo.toml` - Rust workspace configuration
- `.pre-commit-config.yaml` - Pre-commit hook configuration
- `atomi_release.yaml` - Release configuration

### Directory Structure
```
├── cyanprint/          # Main crate
├── cyanprompt/         # Prompt handling crate
├── cyanregistry/       # Registry management crate
├── cyancoordinator/    # Coordination crate
├── nix/               # Nix configuration files
├── scripts/           # Helper scripts
├── .github/           # CI/CD workflows
└── e2e/               # End-to-end tests
```

## LLM-Specific Best Practices

### Before Making Changes
1. **Check existing tasks**: Review `Taskfile.yaml` for available commands
2. **Understand environment**: Check `nix/packages.nix` for available tools
3. **Review pre-commit hooks**: Understand what will be checked via `nix/pre-commit.nix`

### During Development
1. **Use provided tools**: Leverage tools available in the Nix environment
2. **Follow workspace structure**: Work within the established crate organization
3. **Maintain consistency**: Use existing patterns and conventions

### Before Submitting
1. **Always run `pls lint`**: This is non-negotiable
2. **Test your changes**: Use `pls run` to verify functionality
3. **Check build**: Use `pls build` to ensure compilation
4. **Review commit messages**: Follow gitlint conventions

### When Adding New Dependencies
1. **Check if available in Nix**: Look in `nix/packages.nix` first
2. **Add to appropriate Nix file**: Don't bypass the Nix environment
3. **Update flake.lock**: Run `nix flake update` if needed
4. **Test environment**: Ensure `direnv` reloads correctly

## Common Pitfalls to Avoid

1. **Don't skip `pls lint`**: It catches many issues early
2. **Don't install tools outside Nix**: Use the provided environment
3. **Don't ignore the workspace structure**: Follow the multi-crate pattern
4. **Don't commit without testing**: Always verify changes work
5. **Don't bypass pre-commit hooks**: They enforce quality standards

## Quick Reference Commands

```bash
# Setup (if not using direnv)
nix develop

# Common development workflow
pls build        # Build the project
pls run          # Run the application
pls lint         # Run all checks (MANDATORY before completion)

# Manual operations (if needed)
cargo build --release
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Environment Variables

The project uses platform and service variables:
- `PLATFORM=sulfone`
- `SERVICE=iridium`

These are set in `Taskfile.yaml` and used throughout the project.

---

**Remember**: This is a Nix-managed, quality-focused Rust project. Always use the provided tooling and never skip the linting step! 