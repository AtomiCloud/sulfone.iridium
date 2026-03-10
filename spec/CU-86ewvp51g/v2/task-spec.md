# Task Spec: Build + Push Commands (v2)

**Ticket:** CU-86ewvp51g
**Component:** Iridium (cyanprint CLI)
**Parent:** CU-86et8z88g (Local Testing Strategy)

## Summary

Implement `cyanprint build` command and enhance `cyanprint push` with `--build` option for building Docker images and publishing to the CyanPrint registry. This enables CI/CD workflows to build and publish template, processor, plugin, and resolver artifacts.

## v2 Changes (from v1 feedback)

1. **Separate registry from image name**: The `build.registry` field now contains only the registry (e.g., `kirinnee` for Docker Hub, `ghcr.io/org` for GitHub). The image name is specified in each image's `image` field. Final tag: `{registry}/{image}:{tag}`.

2. **`--folder` CLI option**: Add `--folder` option to change working directory before running. This is separate from `--config` which specifies the config file path relative to the folder.

---

## Part 1: Build Configuration

### cyan.yaml Structure (Template)

```yaml
build:
  # Docker registry (just the registry, not the full image path)
  registry: ghcr.io/atomicloud

  # Supported platforms (optional)
  platforms:
    - linux/arm64
    - linux/amd64

  # Docker images to build
  images:
    template:
      image: sulfone.ketone.nix-init/template # Image name (concatenated with registry)
      dockerfile: cyan/template.Dockerfile
      context: cyan
    blob:
      image: sulfone.ketone.nix-init/blob
      dockerfile: cyan/blob.Dockerfile
      context: .
```

### cyan.yaml Structure (Plugin)

```yaml
build:
  registry: kirinnee # Docker Hub
  platforms:
    - linux/arm64
    - linux/amd64
  images:
    plugin:
      image: plugin2 # Final: kirinnee/plugin2:tag
      dockerfile: Dockerfile
      context: .
```

### Config Fields

| Field                 | Type     | Required | Description                                       |
| --------------------- | -------- | -------- | ------------------------------------------------- |
| `registry`            | string   | Yes      | Docker registry (e.g., `kirinnee`, `ghcr.io/org`) |
| `platforms`           | string[] | No       | Target platforms (default: current platform only) |
| `images.*.image`      | string   | Yes      | Image name (concatenated with registry)           |
| `images.*.dockerfile` | string   | Yes      | Path to Dockerfile                                |
| `images.*.context`    | string   | Yes      | Build context directory (relative to folder)      |

---

## Part 2: CLI Options

### `--folder` and `--config` Interaction

| Option     | Description                              | Default     |
| ---------- | ---------------------------------------- | ----------- |
| `--folder` | Working directory to run command from    | `.` (cwd)   |
| `--config` | Config file path, relative to `--folder` | `cyan.yaml` |

```bash
# Run from ./plugin2 folder, read cyan.yaml there
cyanprint build v1.0.0 --folder ./plugin2 --config cyan.yaml

# Run from ./plugin2 folder, read custom config
cyanprint build v1.0.0 --folder ./plugin2 --config build.yaml

# Run from current dir (default), read ./plugin2/cyan.yaml
cyanprint build v1.0.0 --config ./plugin2/cyan.yaml
```

---

## Part 3: Image Tag Resolution (v2)

```
{registry}/{image}:{tag}

Example:
kirinnee/plugin2:v1.0.0
└── registry    └── image name (from images.*.image)    └── tag from CLI
```

---

## Part 4: Build Command

### Usage

```bash
cyanprint build <tag> [options]
```

### Options

| Option                   | Description                                                         |
| ------------------------ | ------------------------------------------------------------------- |
| `--folder <path>`        | Working directory to run from (default: current dir)                |
| `--config <path>`        | Config file path, relative to folder (default: `cyan.yaml`)         |
| `--platform <platforms>` | Target platforms (comma-separated, e.g., `linux/arm64,linux/amd64`) |
| `--builder <name>`       | Buildx builder to use                                               |
| `--no-cache`             | Don't use cache                                                     |
| `--dry-run`              | Show commands without executing                                     |

### Behavior

1. Change to `--folder` directory if specified
2. Read config from `--config` path (relative to folder)
3. Validate `registry` and `image` fields exist
4. Build images: `{registry}/{image}:{tag}`
5. Push to Docker registry

### buildx Command Template

```bash
docker buildx build \
  <context> \
  -f <dockerfile> \
  --platform <platforms> \
  -t <registry>/<image>:<tag> \
  --push \
  [--no-cache]
```

---

## Part 5: Push Command (Enhanced)

### Add `--folder` Option

```bash
cyanprint push plugin --build v1.0.0 --folder ./plugin2 --config cyan.yaml
```

### Options (All Subcommands with --build)

| Option                   | Description                          |
| ------------------------ | ------------------------------------ |
| `--folder <path>`        | Working directory to run from        |
| `--config <path>`        | Config file path, relative to folder |
| `--platform <platforms>` | Target platforms (comma-separated)   |
| `--builder <name>`       | Buildx builder to use                |
| `--no-cache`             | Don't use cache                      |

---

## Files to Modify

### cyanregistry/src/cli/models/build_config.rs

- [ ] Add `image` field to `ImageConfig` struct (required)

### cyanregistry/src/cli/mapper.rs

- [ ] Validate `image` field exists
- [ ] Update image reference construction: `{registry}/{image}:{tag}`

### cyanprint/src/commands.rs

- [ ] Add `--folder` option to `Build` command
- [ ] Add `--folder` option to push subcommands (when using `--build`)

### cyanprint/src/main.rs

- [ ] Handle `--folder` option: change directory before loading config
- [ ] Resolve `--config` path relative to `--folder`

---

## Acceptance Criteria (v2)

1. **Separate registry and image** - `registry: kirinnee` + `image: plugin2` → `kirinnee/plugin2:tag`
2. **`--folder` option** - Changes working directory before running
3. **`--config` relative to folder** - `--folder ./foo --config bar.yaml` reads `./foo/bar.yaml`
4. **Backward compatibility** - Existing push syntax still works

---

## Error Handling (v2)

| Scenario                   | Error Message                           | Exit Code |
| -------------------------- | --------------------------------------- | --------- |
| Missing `image` field      | `Error: images.*.image is required`     | 1         |
| Folder not found           | `Error: folder '{path}' does not exist` | 1         |
| Config not found in folder | `Error: config file '{path}' not found` | 1         |
