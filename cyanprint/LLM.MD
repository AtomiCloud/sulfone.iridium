# CyanPrint - LLM Developer Guide

## Overview
CyanPrint is the main CLI application that orchestrates template operations using a stateless object-oriented architecture with dependency injection. It serves as the top-level coordinator that manages all template lifecycle operations including creation, updates, and registry interactions.

## Architecture Philosophy
- **Stateless OOP with DI**: All objects are stateless and dependencies are injected at the top level
- **Top-Level Control**: The main function controls all object instantiation and dependency wiring
- **Clean Separation**: Clear separation between CLI concerns, coordination logic, and domain operations
- **Error Propagation**: Comprehensive error handling with proper propagation through the call stack

## Folder Structure

```
src/
├── main.rs           # Entry point, DI container, top-level orchestration
├── commands.rs       # CLI command definitions and argument parsing
├── coord.rs          # Coordinator service startup and management
├── run.rs            # Template creation workflow orchestration
├── update.rs         # Template update workflow orchestration
├── util.rs           # Utility functions (parsing, etc.)
└── errors.rs         # Custom error types and error handling
```

## Key Components

### main.rs - Dependency Injection Container
- **Purpose**: Acts as the DI container and application entry point
- **Pattern**: Creates all dependencies at the top level and injects them down
- **Key Dependencies**:
  - `CyanRegistryClient` - Registry operations
  - `CyanCoordinatorClient` - Template execution coordination
  - `DefaultSessionIdGenerator` - Session management
- **Flow**: Parse CLI → Create dependencies → Route to appropriate workflow

### commands.rs - CLI Interface
- **Purpose**: Defines the CLI structure using Clap
- **Pattern**: Declarative command definitions with derive macros
- **Commands**:
  - `Push` - Push templates/processors/plugins to registry
  - `Create` - Create new projects from templates
  - `Update` - Update existing projects with newer template versions

### Workflow Orchestrators

#### run.rs - Template Creation
- **Purpose**: Orchestrates the creation of new projects from templates
- **Dependencies**: All injected from main.rs
- **Flow**: Fetch template → Execute via coordinator → Handle results

#### update.rs - Template Updates
- **Purpose**: Orchestrates updating existing projects to newer template versions
- **Dependencies**: All injected from main.rs
- **Flow**: Discover existing templates → Check for updates → Execute updates

#### coord.rs - Coordinator Management
- **Purpose**: Manages the lifecycle of the coordinator service
- **Pattern**: Service management with proper startup/shutdown

## Dependency Management

### External Dependencies
```toml
# Core Rust ecosystem
clap = "4.5.38"           # CLI parsing
serde = "1.0"             # Serialization
tokio = "1"               # Async runtime
reqwest = "0.12"          # HTTP client

# Domain-specific
bollard = "*"             # Docker API
inquire = "0.7.5"         # Interactive prompts
uuid = "1.16.0"           # UUID generation
chrono = "0.4"            # Date/time handling

# Internal workspace dependencies
cyanprompt = { path = "../cyanprompt" }
cyanregistry = { path = "../cyanregistry" }
cyancoordinator = { path = "../cyancoordinator" }
```

### Internal Dependencies
- **cyanprompt**: User interaction and prompting
- **cyanregistry**: Registry client and operations
- **cyancoordinator**: Template execution coordination

## Design Patterns

### 1. Dependency Injection at Top Level
```rust
// All dependencies created in main.rs
let http_client = new_client()?;
let registry = CyanRegistryClient { ... };
let coord_client = CyanCoordinatorClient::new(...);
let session_id_generator = Box::new(DefaultSessionIdGenerator);

// Injected into workflow functions
cyan_run(session_id_generator, path, template, coord_client, ...)
```

### 2. Stateless Operations
- No mutable state stored in structs
- All state passed as parameters
- Operations are pure functions where possible

### 3. Error Handling
- Comprehensive error propagation using `Result<T, Box<dyn Error + Send>>`
- Graceful error handling with user-friendly messages
- Session cleanup on both success and failure paths

### 4. Resource Management
- Proper session cleanup after operations
- Resource lifecycle management (Docker containers, temporary files)

## Usage Patterns

### Creating New Projects
```bash
cyanprint create username/template-name:version ./target-path
```

### Updating Existing Projects
```bash
cyanprint update ./project-path --interactive
```

### Publishing Templates
```bash
cyanprint push template --config cyan.yaml --token TOKEN
```

## Integration Points

### Registry Integration
- Fetches templates, processors, and plugins
- Handles authentication via tokens
- Version resolution and management

### Coordinator Integration
- Template execution orchestration
- Session management and cleanup
- Docker container lifecycle management

### Prompt Integration
- Interactive user prompts for template parameters
- Answer collection and validation
- Skip/default value handling

## Development Guidelines

1. **Maintain Stateless Design**: Keep all objects stateless with dependencies injected
2. **Top-Level Control**: All dependency creation should happen in main.rs
3. **Error Propagation**: Always propagate errors properly with context
4. **Resource Cleanup**: Ensure proper cleanup of sessions and resources
5. **CLI Consistency**: Maintain consistent CLI patterns and help text
6. **Integration Testing**: Test full workflows end-to-end

## Testing Strategy
- Unit tests for utility functions
- Integration tests for workflow orchestration
- End-to-end tests for CLI commands
- Mock external dependencies (registry, coordinator) for isolated testing

## Future Considerations
- Configuration file support for default registry endpoints
- Plugin system for custom template operations
- Caching layer for template metadata
- Parallel template operations for bulk updates 