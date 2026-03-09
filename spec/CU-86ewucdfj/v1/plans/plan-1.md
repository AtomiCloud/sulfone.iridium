# Plan 1: Daemon Subcommand Group with Stop/Cleanup

**Goal**: Convert `cyanprint daemon` to a subcommand group and add `daemon stop` command for cleanup and container removal.

## Files to Modify

| File                                       | Changes                                                               |
| ------------------------------------------ | --------------------------------------------------------------------- |
| `cyanprint/src/commands.rs`                | Convert `Daemon` to subcommand enum with `Start` and `Stop` variants  |
| `cyanprint/src/main.rs`                    | Handle `Daemon::Start` and `Daemon::Stop` command variants            |
| `cyanprint/src/coord.rs`                   | Add `stop_coordinator()` async function                               |
| `cyancoordinator/src/client.rs`            | Add `cleanup()` method                                                |
| `e2e/setup.sh`                             | Change `cyanprint daemon` to `cyanprint daemon start`                 |
| `docs/developer/surfaces/cli/04-daemon.md` | Update documentation for subcommand structure, add `daemon stop` docs |

## Implementation Approach

### Step 1: Update CLI Commands (`commands.rs`)

Convert the `Daemon` variant from a simple command to a subcommand group:

```rust
// Before: simple variant
Commands::Daemon { version, port, registry }

// After: subcommand group
Commands::Daemon {
    #[command(subcommand)]
    command: DaemonCommands,
}

#[derive(Subcommand)]
pub enum DaemonCommands {
    #[command(about = "Start the CyanPrint Coordinator daemon")]
    Start {
        #[arg(value_name = "COORDINATOR_VERSION", default_value = "latest")]
        version: String,
        #[arg(short, long, default_value = "9000")]
        port: u16,
        #[arg(short, long, env = "CYANPRINT_REGISTRY")]
        registry: Option<String>,
    },
    #[command(about = "Stop the CyanPrint Coordinator daemon and cleanup")]
    Stop {
        #[arg(short, long, default_value = "9000", help = "Port where daemon is running")]
        port: u16,
    },
}
```

### Step 2: Add Cleanup Method (`cyancoordinator/src/client.rs`)

Add a `cleanup()` method following the existing `clean()` pattern:

```rust
pub fn cleanup(&self) -> Result<CleanupRes, Box<dyn Error + Send>> {
    let host = self.endpoint.to_string();
    let endpoint = host + "/cleanup";
    let http_client = new_client()?;
    http_client
        .delete(endpoint)
        .send()
        .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
        .and_then(|x| {
            if x.status().is_success() {
                x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>)
            } else {
                // Handle error like existing methods
            }
        })
}
```

Define `CleanupRes` struct to match Boron's response:

- `removed_containers: Vec<String>`
- `removed_images: Vec<String>`
- `removed_volumes: Vec<String>`

### Step 3: Add Stop Coordinator Function (`coord.rs`)

Add `stop_coordinator()` async function:

```rust
pub async fn stop_coordinator(docker: Docker, port: u16) -> Result<(), Box<dyn Error + Send>> {
    let coord = "cyanprint-coordinator";

    // 1. Call DELETE /cleanup on the Boron container
    println!("🧹 Calling cleanup endpoint on coordinator...");
    let client = CyanCoordinatorClient::new(format!("http://localhost:{}", port));
    match client.cleanup() {
        Ok(res) => {
            println!("✅ Cleanup completed");
            // Optionally print removed resources
        }
        Err(e) => {
            eprintln!("⚠️ Cleanup endpoint failed: {}", e);
            // Continue to container removal anyway
        }
    }

    // 2. Find and remove the coordinator container
    println!("🔍 Looking for coordinator container...");
    let containers = docker.list_containers(Some(ListContainersOptions {
        all: true,
        filters: Some({("name": [coord])}),
        ..Default::default()
    })).await?;

    if containers.is_empty() {
        println!("✅ No coordinator container found");
        return Ok(());
    }

    for container in containers {
        if let Some(id) = &container.id {
            println!("🗑️ Removing container: {}", id);
            docker.remove_container(id, Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            })).await?;
            println!("✅ Container removed");
        }
    }

    Ok(())
}
```

### Step 4: Update Main Handler (`main.rs`)

Handle the new subcommand variants:

```rust
Commands::Daemon { command } => {
    let docker = Docker::connect_with_local_defaults()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            match command {
                DaemonCommands::Start { version, port, registry } => {
                    let img = format!("ghcr.io/atomicloud/sulfone.boron/sulfone-boron:{}", version);
                    match start_coordinator(docker, img, port, registry).await {
                        Ok(_) => println!("✅ Coordinator started on port {}", port),
                        Err(e) => eprintln!("🚨 Error: {:?}", e),
                    }
                }
                DaemonCommands::Stop { port } => {
                    match stop_coordinator(docker, port).await {
                        Ok(_) => println!("✅ Coordinator stopped"),
                        Err(e) => eprintln!("🚨 Error: {:?}", e),
                    }
                }
            }
        });
    Ok(())
}
```

### Step 5: Update E2E Script (`e2e/setup.sh`)

Change line 10:

```bash
# Before
cyanprint daemon

# After
cyanprint daemon start
```

### Step 6: Update Documentation (`docs/developer/surfaces/cli/04-daemon.md`)

Update the daemon command documentation to reflect the new subcommand structure:

1. **Update usage section**:

   - Change `pls daemon [version] [options]` to `pls daemon <start|stop> [options]`

2. **Add `daemon start` section**:

   - Document the renamed start command
   - Keep existing examples with updated command

3. **Add `daemon stop` section**:

   - Document the new stop command
   - Options: `--port`
   - Examples showing cleanup and container removal
   - Flow diagram for stop process

4. **Update flow diagrams** to show both start and stop flows

## Edge Cases to Handle

1. **Daemon not running**: `stop_coordinator` should handle gracefully when no container exists
2. **Cleanup endpoint fails**: Log warning but continue with container removal
3. **Container in stopped state**: Use `force: true` to remove regardless of state
4. **Network cleanup**: NOT removing the Docker network (leave for faster subsequent starts)

## Testing Strategy

1. **Manual testing**:

   - `cyanprint daemon start` - verify existing behavior
   - `cyanprint daemon stop` - verify cleanup is called and container removed
   - `cyanprint daemon stop` when not running - verify graceful handling

2. **E2E testing**:
   - Run `e2e/setup.sh` to verify it still works with new command structure

## Implementation Checklist

- [ ] Convert `Daemon` to subcommand group in `commands.rs`
- [ ] Add `DaemonCommands` enum with `Start` and `Stop` variants
- [ ] Add `cleanup()` method to `CyanCoordinatorClient`
- [ ] Add `CleanupRes` response struct
- [ ] Add `stop_coordinator()` function to `coord.rs`
- [ ] Update `main.rs` to handle `DaemonCommands`
- [ ] Update `e2e/setup.sh` line 10
- [ ] Update `docs/developer/surfaces/cli/04-daemon.md` with subcommand structure
- [ ] Test `cyanprint daemon start` (existing behavior)
- [ ] Test `cyanprint daemon stop` (new behavior)
- [ ] Test `cyanprint daemon stop` when not running
