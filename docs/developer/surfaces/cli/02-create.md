# create Command

**Key File**: `cyanprint/src/main.rs:131-191`

## Usage

```bash
pls create <template_ref> [path] [options]
```

## Description

Creates a new project from a template. The template is fetched from the registry and executed in the coordinator service.

## Arguments

| Argument         | Required | Description                                                |
| ---------------- | -------- | ---------------------------------------------------------- |
| `<template_ref>` | Yes      | Template reference in format `<username>/<name>:<version>` |
| `[path]`         | No       | Destination directory (default: current directory)         |

## Options

| Option                   | Short | Default                           | Description                  |
| ------------------------ | ----- | --------------------------------- | ---------------------------- |
| `--coordinator-endpoint` | `-c`  | `http://coord.cyanprint.dev:9000` | Coordinator service endpoint |

**Environment Variable**: `CYANPRINT_COORDINATOR`

**Key File**: `cyanprint/src/commands.rs:32-46`

## Examples

### Basic Usage

```bash
pls create atomicloud/starter:1 ./my-project
```

Output:

```text
🚘 Retrieving template 'atomicloud/starter:1' from registry...
✅ Retrieved template 'atomicloud/starter:1' from registry.
✅ Completed successfully
🧹 Cleaning up all sessions...
✅ Cleaned up all sessions
```

### With Default Coordinator

```bash
pls create atomicloud/starter:1 ./my-project
# Uses default coordinator: http://coord.cyanprint.dev:9000
```

### With Custom Coordinator

```bash
pls create atomicloud/starter:1 ./my-project --coordinator-endpoint http://localhost:9000
```

## Flow

```mermaid
sequenceDiagram
    participant U as User
    participant CLI as cyanprint
    participant REG as Registry
    participant COORD as Coordinator
    participant FS as Filesystem

    U->>CLI: 1. pls create template:version ./path
    CLI->>REG: 2. GET /template
    REG-->>CLI: 3. Template metadata
    CLI->>COORD: 4. Bootstrap session
    CLI->>COORD: 5. Execute template
    COORD-->>CLI: 6. Archive data
    CLI->>FS: 7. Unpack & merge
    CLI->>COORD: 8. Clean session
```

| Order | Step              | What                              | Key File              |
| ----- | ----------------- | --------------------------------- | --------------------- |
| 1     | Parse command     | Parse template reference and path | `commands.rs:34`      |
| 2     | Fetch template    | Get template from registry        | `main.rs:142-157`     |
| 3     | Parse reference   | Extract username, name, version   | `util.rs:parse_ref()` |
| 4     | Bootstrap session | Initialize coordinator session    | `main.rs:142-157`     |
| 5     | Execute template  | Run template with coordinator     | `run.rs:cyan_run()`   |
| 6     | Receive archive   | Get generated files from coord    | `run.rs:cyan_run()`   |
| 7     | Unpack & merge    | Write files to filesystem         | `run.rs:49-52`        |
| 8     | Cleanup           | Remove session artifacts          | `main.rs:179-181`     |

## Template Reference Format

```text
<username>/<template_name>:<version>
```

Components:

- `username` - Template author/organization
- `template_name` - Template identifier
- `version` - Version number (integer)

**Key File**: `cyanprint/src/util.rs` → `parse_ref()`

## Exit Codes

| Code | Meaning                           |
| ---- | --------------------------------- |
| `0`  | Success                           |
| `1`  | General error                     |
| `2`  | Invalid template reference format |

## Related Commands

- [`update`](./03-update.md) - Update existing template
- [`push`](./01-push.md) - Publish new template
- [`daemon`](./04-daemon.md) - Start coordinator service
