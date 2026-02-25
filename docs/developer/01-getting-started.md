# Getting Started

## Prerequisites

- **Rust** 1.70 or later
- **Docker** - For running the coordinator daemon
- **Nix** (optional) - For development environment

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/AtomiCloud/sulfone.iridium
cd sulfone.iridium

# Build the CLI
cargo build --release

# The binary will be at target/release/cyanprint
```

### Using Nix

```bash
# Enter the development environment (Nix flakes)
nix develop

# Or using direnv (automatic)
direnv allow

# Build
cargo build --release
```

## Quick Start

> **Note on command entry points**: Examples in this guide use two command entry points:
>
> - `cyanprint` — the installed binary (production/release build)
> - `pls` — a development alias that runs the CLI via `cargo run` from the current source tree
>
> Use `pls` for local development and `cyanprint` for installed/production usage.

### 1. Start the Coordinator Daemon

The coordinator daemon is required for template execution.

```bash
# Start the coordinator on port 9000
cyanprint daemon --version latest --port 9000 --registry https://api.zinc.sulfone.raichu.cluster.atomi.cloud
```

Expected output:

```text
✅ Coordinator started on port 9000
```

**Key File**: `cyanprint/src/coord.rs` → `start_coordinator()`

### 2. Create a Project from Template

```bash
# Create a new project using a template
pls create <username>/<template-name>:<version> <destination-path>
```

Example:

```bash
pls create atomicloud/starter:1 ./my-project
```

Expected output:

```text
🚘 Retrieving template 'atomicloud/starter:1' from registry...
✅ Retrieved template 'atomicloud/starter:1' from registry.
✅ Completed successfully
🧹 Cleaning up all sessions...
✅ Cleaned up all sessions
```

**Key File**: `cyanprint/src/run.rs` → `cyan_run()`

### 3. Update Existing Templates

```bash
# Update templates in an existing project to latest versions
pls update ./my-project
```

Expected output:

```text
🔄 Updating templates to latest versions
✅ Update completed successfully
🧹 Cleaning up all sessions...
✅ Cleaned up all sessions
```

**Key File**: `cyanprint/src/update.rs` → `cyan_update()`

### 4. Push a Template

```bash
# Push a template to the registry
pls push template \
  --config path/to/cyan.yaml \
  --token YOUR_TOKEN \
  --message "Initial release" \
  --template-image registry/username/template:latest \
  --template-tag latest \
  --blob-image registry/username/blobs:latest \
  --blob-tag latest
```

Expected output:

```text
✅ Pushed template successfully
📦 Template ID: 12345
```

**Key File**: `cyanregistry/src/http/client.rs` → `push_template()`

## Configuration

The coordinator and registry endpoints can be configured via:

| Option          | Default                                               | Description           |
| --------------- | ----------------------------------------------------- | --------------------- |
| `--registry`    | `https://api.zinc.sulfone.raichu.cluster.atomi.cloud` | Registry API endpoint |
| `--coordinator` | `http://coord.cyanprint.dev:9000`                     | Coordinator endpoint  |

## Project Structure

```text
iridium/
├── cyanprint/           # CLI binary
│   └── src/
│       ├── main.rs      # Entry point
│       ├── commands.rs  # CLI definitions
│       ├── run.rs       # Template execution
│       ├── update.rs    # Template updates
│       └── coord.rs     # Coordinator startup
├── cyancoordinator/     # Core engine
│   └── src/
│       ├── lib.rs       # Module exports
│       ├── client.rs    # HTTP client
│       ├── fs/          # Virtual file system
│       ├── operations/  # Template operations
│       ├── session/     # Session management
│       ├── state/       # State persistence
│       └── template/    # Template execution
├── cyanprompt/          # Prompting engine
│   └── src/
│       ├── lib.rs
│       ├── domain/      # Domain services
│       └── http/        # HTTP client
└── cyanregistry/        # Registry client
    └── src/
        ├── lib.rs
        ├── domain/      # Domain models
        ├── http/        # HTTP client
        └── cli/         # CLI models
```

## Common Issues

### Issue: Coordinator not reachable

**Symptom**: `Error: Failed to connect to coordinator`

**Solution**: Ensure the coordinator daemon is running:

```bash
# Check if coordinator is running
curl http://localhost:9000/health

# Start if not running
cyanprint daemon --version latest --port 9000
```

### Issue: Template not found

**Symptom**: `Error: Template not found in registry`

**Solution**:

1. Verify the template reference format: `<username>/<name>:<version>`
2. Check that the template exists in the registry
3. Ensure you have access to private templates

### Issue: Docker not running

**Symptom**: `Error: Failed to connect to Docker daemon`

**Solution**: Start Docker:

```bash
# macOS
open -a Docker

# Linux
sudo systemctl start docker
```

## Next Steps

- [Architecture](./02-architecture.md) - System overview
- [Concepts](./concepts/) - Domain terminology
- [CLI Commands](./surfaces/cli/) - Detailed command reference
