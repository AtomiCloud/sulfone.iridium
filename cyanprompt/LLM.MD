# CyanPrompt - LLM Developer Guide

## Overview
CyanPrompt is a domain-focused library that handles user interaction and prompting functionality. It provides a clean abstraction over interactive CLI prompts with support for various input types, validation, and answer serialization. The library follows a stateless OOP design with clear separation between domain models and HTTP/transport concerns.

## Architecture Philosophy
- **Domain-Driven Design**: Clear separation between domain logic and transport/HTTP concerns
- **Stateless Services**: All services are stateless functions that operate on data
- **Type Safety**: Strong typing for all prompt types and answers
- **Serialization-Ready**: All models are serializable for persistence and transport

## Folder Structure

```
src/
├── lib.rs                    # Library entry point and module exports
├── domain/                   # Domain logic and models
│   ├── mod.rs               # Domain module exports
│   ├── models/              # Domain data models
│   │   ├── mod.rs          # Model exports
│   │   ├── answer.rs       # Answer enum and implementations
│   │   ├── question.rs     # Question types and validation
│   │   ├── prompt.rs       # Prompt type definitions
│   │   ├── cyan.rs         # Cyan-specific domain models
│   │   └── template/       # Template-related models
│   └── services/           # Domain services (business logic)
│       ├── mod.rs          # Service exports
│       ├── prompter.rs     # Core prompting service
│       ├── repo.rs         # Repository pattern implementations
│       └── template/       # Template-specific services
└── http/                   # HTTP/transport layer
    ├── mod.rs              # HTTP module exports
    ├── client.rs           # HTTP client implementations
    ├── mapper.rs           # Domain ↔ HTTP mapping
    ├── core/               # Core HTTP functionality
    ├── template/           # Template HTTP operations
    └── extension/          # HTTP extensions and middleware
```

## Key Components

### Domain Models

#### answer.rs - Answer Types
```rust
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Answer {
    String(String),
    StringArray(Vec<String>),
    Bool(bool),
}
```
- **Purpose**: Represents user answers to prompts
- **Pattern**: Tagged union for type-safe answer handling
- **Serialization**: Serde-compatible for persistence

#### question.rs - Question Definitions
- **Purpose**: Defines question types and validation rules
- **Pattern**: Type-safe question definitions with validation
- **Features**: Required/optional questions, default values, validation rules

#### prompt.rs - Prompt Types
- **Purpose**: Defines the various prompt types available
- **Pattern**: Enum-based prompt type system
- **Types**: Text, Confirm, Checkbox, Select, Password, Date

### Domain Services

#### prompter.rs - Core Prompting Service
```rust
pub fn prompt(p: Prompts) -> Result<Option<Answer>, Box<dyn std::error::Error + Send>> {
    match p {
        Prompts::Text(text) => text.prompt_skippable().map(|a| a.map(Answer::String)),
        Prompts::Confirm(cfm) => cfm.prompt_skippable().map(|a| a.map(Answer::Bool)),
        // ... other prompt types
    }
}
```
- **Purpose**: Stateless service function for executing prompts
- **Pattern**: Pattern matching on prompt types
- **Features**: Skippable prompts, error handling, type conversion

### HTTP Layer

#### client.rs - HTTP Client
- **Purpose**: Handles HTTP communications for prompt-related operations
- **Pattern**: Client struct with injected dependencies
- **Features**: Request/response handling, error mapping

#### mapper.rs - Domain/HTTP Mapping
- **Purpose**: Maps between domain models and HTTP DTOs
- **Pattern**: Pure mapping functions
- **Features**: Bidirectional mapping, validation, error handling

## Design Patterns

### 1. Domain-Driven Design
```rust
// Clear separation between domain and transport
src/domain/     # Pure domain logic
src/http/       # HTTP transport concerns
```

### 2. Stateless Services
```rust
// Services are pure functions
pub fn prompt(p: Prompts) -> Result<Option<Answer>, Error> {
    // No instance state, pure function
}
```

### 3. Type-Safe Enums
```rust
// Strongly typed prompt and answer systems
pub enum Prompts { Text(TextPrompt), Confirm(ConfirmPrompt), ... }
pub enum Answer { String(String), Bool(bool), ... }
```

### 4. Serialization-First Design
```rust
#[derive(Clone, Serialize, Deserialize)]
// All models are serializable by default
```

## Dependencies

### External Dependencies
```toml
# Core functionality
inquire = { version = "0.7.5", features = ["date"] }  # Interactive prompts
serde = { version = "1.0", features = ["derive"] }    # Serialization
chrono = { version = "0.4", features = ["unstable-locales"] }  # Date handling

# HTTP/Network
reqwest = { version = "0.12", features = ["json", "blocking"] }
tokio = { version = "1", features = ["full"] }        # Async runtime
serde_json = "1.0"                                     # JSON serialization
```

### Key Libraries
- **inquire**: Provides the actual CLI prompting functionality
- **serde**: Handles serialization/deserialization of all models
- **chrono**: Date/time handling for date prompts
- **reqwest**: HTTP client for remote operations

## Usage Patterns

### Basic Prompting
```rust
use cyanprompt::domain::services::prompter::prompt;
use cyanprompt::domain::models::prompt::Prompts;

let text_prompt = TextPrompt::new("Enter your name:");
let answer = prompt(Prompts::Text(text_prompt))?;
```

### Answer Handling
```rust
match answer {
    Some(Answer::String(s)) => println!("Got string: {}", s),
    Some(Answer::Bool(b)) => println!("Got bool: {}", b),
    None => println!("User skipped"),
}
```

### Serialization
```rust
// All answers are serializable
let json = serde_json::to_string(&answer)?;
let answer: Answer = serde_json::from_str(&json)?;
```

## Integration Points

### Template Systems
- Provides prompting for template parameters
- Handles answer collection and validation
- Supports default values and skip behavior

### HTTP Services
- Maps domain models to/from HTTP DTOs
- Handles remote prompt operations
- Manages HTTP client lifecycle

### Persistence
- All models are serializable for storage
- Supports answer caching and replay
- Compatible with various storage backends

## Development Guidelines

1. **Domain First**: Always start with domain models and services
2. **Stateless Design**: Keep all services as pure functions
3. **Type Safety**: Use strong typing for all prompt and answer types
4. **Serialization**: Ensure all models are serializable
5. **Error Handling**: Provide meaningful error messages for prompt failures
6. **Validation**: Implement proper validation for all input types

## Testing Strategy

### Unit Testing
- Test individual prompt types
- Test answer serialization/deserialization
- Test validation logic

### Integration Testing
- Test full prompt workflows
- Test HTTP client operations
- Test mapper functions

### User Experience Testing
- Test prompt UX with real users
- Validate error messages and help text
- Test skip/default behavior

## Extension Points

### Custom Prompt Types
- Extend the `Prompts` enum for new prompt types
- Implement corresponding answer types
- Add mapper functions for HTTP transport

### Validation Rules
- Add custom validation for specific domains
- Implement complex validation logic
- Support conditional validation

### HTTP Extensions
- Add custom HTTP headers
- Implement authentication mechanisms
- Support custom serialization formats

## Future Considerations
- Plugin system for custom prompt types
- Advanced validation with regex/custom rules
- Multi-language support for prompts
- Rich text formatting for complex prompts
- Integration with external validation services 