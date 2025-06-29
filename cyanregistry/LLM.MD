# CyanRegistry - LLM Developer Guide

## Overview
CyanRegistry is a comprehensive client library for interacting with the Cyan template registry. It provides both HTTP client functionality and CLI configuration management. The library follows a layered architecture with clear separation between domain models, HTTP transport, and CLI concerns, all implemented using stateless OOP with dependency injection.

## Architecture Philosophy
- **Layered Architecture**: Clear separation between domain, HTTP, and CLI layers
- **Stateless OOP with DI**: All clients are stateless with injected dependencies
- **Configuration-Driven**: YAML-based configuration with strong typing
- **HTTP-First**: Designed around RESTful HTTP API interactions
- **Error Transparency**: Comprehensive error handling with detailed problem reporting

## Folder Structure

```
src/
├── lib.rs                    # Library entry point and module exports
├── domain/                   # Domain logic and configuration models
│   ├── mod.rs               # Domain module exports
│   └── config/              # Configuration domain models
│       ├── mod.rs          # Configuration exports
│       ├── template_config.rs    # Template configuration models
│       ├── processor_config.rs   # Processor configuration models
│       └── plugin_config.rs      # Plugin configuration models
├── http/                    # HTTP client and transport layer
│   ├── mod.rs              # HTTP module exports
│   ├── client.rs           # Main HTTP client implementation
│   ├── mapper.rs           # Domain ↔ HTTP DTO mapping
│   ├── errors.rs           # HTTP-specific error types
│   └── models/             # HTTP request/response models
│       ├── mod.rs          # HTTP model exports
│       ├── template_req.rs # Template request models
│       ├── template_res.rs # Template response models
│       ├── processor_req.rs # Processor request models
│       ├── processor_res.rs # Processor response models
│       ├── plugin_req.rs   # Plugin request models
│       └── plugin_res.rs   # Plugin response models
└── cli/                    # CLI configuration and mapping
    ├── mod.rs              # CLI module exports
    ├── mapper.rs           # Config file ↔ domain mapping
    └── models/             # CLI configuration file models
        ├── mod.rs          # CLI model exports
        ├── template_config.rs  # Template YAML config
        ├── processor_config.rs # Processor YAML config
        └── plugin_config.rs    # Plugin YAML config
```

## Key Components

### HTTP Client Layer

#### client.rs - Registry HTTP Client
```rust
pub struct CyanRegistryClient {
    pub endpoint: String,
    pub version: String,
    pub client: Rc<reqwest::blocking::Client>,
}
```

**Key Features**:
- **Stateless Design**: No mutable state, dependencies injected via constructor
- **Resource Management**: Shared HTTP client via `Rc<reqwest::Client>`
- **Error Handling**: Comprehensive error mapping with `ProblemDetails`
- **Authentication**: Token-based authentication for push operations

**Core Operations**:
- `push_template()` - Upload templates to registry
- `push_processor()` - Upload processors to registry  
- `push_plugin()` - Upload plugins to registry
- `get_template()` - Fetch template by name/version
- `get_processor()` - Fetch processor by name/version
- `get_plugin()` - Fetch plugin by name/version

### HTTP Models

#### Request Models (template_req.rs, processor_req.rs, plugin_req.rs)
- **Purpose**: Strongly typed HTTP request payloads
- **Pattern**: Serde-serializable structs for API requests
- **Validation**: Built-in validation using the `validator` crate

#### Response Models (template_res.rs, processor_res.rs, plugin_res.rs)
- **Purpose**: Strongly typed HTTP response payloads
- **Pattern**: Serde-deserializable structs for API responses
- **Structure**: Hierarchical models with principal/version separation

### CLI Configuration Layer

#### CLI Models (cli/models/)
```rust
#[derive(Deserialize)]
pub struct CyanTemplateFileConfig {
    pub username: String,
    pub name: String,
    pub processors: Vec<String>,
    pub plugins: Vec<String>,
    // ... other fields
}
```

**Features**:
- **YAML Configuration**: Direct mapping from YAML configuration files
- **Type Safety**: Strong typing for all configuration options
- **Validation**: Built-in validation for required fields

#### Mapping Layer (cli/mapper.rs)
- **Purpose**: Maps between CLI config files and domain models
- **Pattern**: Pure mapping functions with error handling
- **Features**: YAML parsing, validation, error reporting

### Domain Layer

#### Configuration Models (domain/config/)
- **Purpose**: Domain representation of configuration data
- **Pattern**: Clean domain models separate from file format
- **Features**: Business logic validation, domain rules

## Design Patterns

### 1. Layered Architecture
```rust
CLI Layer    → YAML configs, file parsing
Domain Layer → Business models, validation  
HTTP Layer   → API client, request/response
```

### 2. Stateless HTTP Client
```rust
// Client with injected dependencies
let client = CyanRegistryClient {
    endpoint: "https://registry.example.com".to_string(),
    version: "1.0".to_string(),
    client: Rc::new(reqwest_client),
};

// All operations are stateless
let result = client.get_template(username, name, version)?;
```

### 3. Comprehensive Error Handling
```rust
// Detailed error types
pub enum GenericError {
    ProblemDetails(ProblemDetails),
    // ... other error types
}

// Error mapping from HTTP responses
if response.status().is_success() {
    response.json()
} else {
    let problem: ProblemDetails = response.json()?;
    Err(GenericError::ProblemDetails(problem))
}
```

### 4. Configuration Mapping Pipeline
```rust
YAML File → CLI Model → Domain Model → HTTP Request → API
```

## Dependencies

### External Dependencies
```toml
# Serialization and validation
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9.34"
validator = { version = "0.20", features = ["derive"] }

# HTTP client
reqwest = { version = "0.12", features = ["json", "blocking"] }
tokio = { version = "1", features = ["full"] }

# Date/time handling
chrono = { version = "0.4", features = ["unstable-locales"] }
```

### Key Libraries
- **reqwest**: HTTP client for registry API calls
- **serde**: Serialization/deserialization for all models
- **validator**: Configuration validation
- **serde_yaml**: YAML configuration file parsing

## Usage Patterns

### Client Initialization
```rust
use cyanregistry::http::client::CyanRegistryClient;
use std::rc::Rc;

let http_client = Rc::new(reqwest::blocking::Client::new());
let registry = CyanRegistryClient {
    endpoint: "https://registry.example.com".to_string(),
    version: "1.0".to_string(),
    client: http_client,
};
```

### Template Operations
```rust
// Get template
let template = registry.get_template(
    "username".to_string(),
    "template-name".to_string(), 
    Some(42) // version
)?;

// Push template
let result = registry.push_template(
    "cyan.yaml".to_string(),
    "auth-token".to_string(),
    "Release description".to_string(),
    "blob-image".to_string(),
    "blob-tag".to_string(),
    "template-image".to_string(),
    "template-tag".to_string(),
)?;
```

### Configuration Handling
```rust
use cyanregistry::cli::mapper::{read_yaml, template_config_mapper};

// Read and parse YAML config
let config: CyanTemplateFileConfig = read_yaml("cyan.yaml")?;

// Map to domain model
let domain_config = template_config_mapper(&config)?;
```

## Integration Points

### Template Systems
- Provides template metadata and versioning
- Handles template discovery and resolution
- Manages template dependencies (processors, plugins)

### CLI Tools
- Configuration file parsing and validation
- Push/pull operations for templates
- Version management and listing

### Build Systems
- Integration with Docker for image management
- CI/CD pipeline integration
- Automated template publishing

## Development Guidelines

1. **Stateless Design**: Keep all clients stateless with injected dependencies
2. **Layer Separation**: Maintain clear boundaries between CLI, domain, and HTTP layers
3. **Error Handling**: Provide detailed error information with context
4. **Configuration Validation**: Validate all configuration at parse time
5. **HTTP Best Practices**: Follow RESTful patterns and proper status code handling
6. **Type Safety**: Use strong typing throughout the entire pipeline

## Testing Strategy

### Unit Testing
- Test individual mapper functions
- Test configuration parsing and validation
- Test error handling scenarios

### Integration Testing
- Test full HTTP client operations
- Test configuration file processing
- Test error response handling

### Contract Testing
- Validate API request/response formats
- Test compatibility with registry API
- Validate configuration schema

## Error Handling

### Error Types
- **HTTP Errors**: Network, timeout, authentication failures
- **Configuration Errors**: Invalid YAML, missing fields, validation failures
- **Domain Errors**: Business rule violations, dependency conflicts
- **Serialization Errors**: JSON/YAML parsing failures

### Error Reporting
```rust
// Detailed problem details from API
#[derive(Deserialize)]
pub struct ProblemDetails {
    pub title: String,
    pub detail: Option<String>,
    pub status: i32,
    // ... other fields
}
```

## Extension Points

### Custom Configuration
- Add new configuration fields
- Implement custom validation rules
- Support additional file formats

### API Extensions
- Add new endpoint support
- Implement custom authentication schemes
- Support additional HTTP methods

### Client Customization
- Custom HTTP middleware
- Request/response interceptors
- Custom serialization formats

## Future Considerations
- Async HTTP client support
- Caching layer for frequently accessed templates
- Batch operations for multiple templates
- Plugin system for custom registry backends
- GraphQL API support alongside REST
- Advanced authentication mechanisms (OAuth, JWT) 