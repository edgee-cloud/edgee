# Edgee Rust SDK

A modern, idiomatic Rust SDK for the [Edgee AI Gateway](https://edgee.ai).

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

## Features

- **🦀 Idiomatic Rust** - Leverages Rust's type system, ownership, and error handling
- **⚡ Async/Await** - Built on tokio for efficient async operations
- **🔒 Type-Safe** - Strong typing with enums, structs, and comprehensive error types
- **📡 Streaming** - First-class support for streaming responses with `Stream` trait
- **🛠️ Tool Calling** - Full support for function/tool calling
- **🎯 Flexible Input** - Accept strings, message arrays, or structured objects
- **🚀 Zero-Cost Abstractions** - Efficient implementation with minimal overhead
- **📦 Minimal Dependencies** - Only essential, well-maintained dependencies

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
edgee = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

```rust
use edgee::Edgee;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client from environment variables (EDGEE_API_KEY)
    let client = Edgee::from_env()?;

    // Simple text completion
    let response = client.send("gpt-4o", "Hello, world!").await?;
    println!("{}", response.text().unwrap_or(""));

    Ok(())
}
```

## Configuration

The SDK supports multiple ways to configure the client:

### From Environment Variables

Set `EDGEE_API_KEY` (required) and optionally `EDGEE_BASE_URL`:

```rust
let client = Edgee::from_env()?;
```

### With API Key

```rust
let client = Edgee::with_api_key("your-api-key");
```

### With Custom Configuration

```rust
use edgee::EdgeeConfig;

let config = EdgeeConfig::new("your-api-key")
    .with_base_url("https://custom.api.url");

let client = Edgee::new(config);
```

## Usage Examples

### Simple Text Completion

```rust
use edgee::Edgee;

let client = Edgee::from_env()?;
let response = client.send("gpt-4o", "Explain Rust in one sentence").await?;
println!("{}", response.text().unwrap_or(""));
```

### Multi-turn Conversation

```rust
use edgee::{Edgee, Message};

let client = Edgee::from_env()?;

let messages = vec![
    Message::system("You are a helpful assistant."),
    Message::user("What's the capital of France?"),
];

let response = client.send("gpt-4o", messages).await?;
println!("{}", response.text().unwrap_or(""));
```

### Streaming Responses

```rust
use edgee::Edgee;
use tokio_stream::StreamExt;

let client = Edgee::from_env()?;
let mut stream = client.stream("gpt-4o", "Tell me a story").await?;

while let Some(chunk) = stream.next().await {
    if let Ok(chunk) = chunk {
        if let Some(text) = chunk.text() {
            print!("{}", text);
        }
    }
}
```

### Tool/Function Calling

```rust
use edgee::{Edgee, Message, InputObject, Tool, FunctionDefinition, JsonSchema};
use std::collections::HashMap;

let client = Edgee::from_env()?;

// Define a function
let function = FunctionDefinition {
    name: "get_weather".to_string(),
    description: Some("Get the weather for a location".to_string()),
    parameters: JsonSchema {
        schema_type: "object".to_string(),
        properties: Some({
            let mut props = HashMap::new();
            props.insert("location".to_string(), serde_json::json!({
                "type": "string",
                "description": "The city and state"
            }));
            props
        }),
        required: Some(vec!["location".to_string()]),
        description: None,
    },
};

// Send request with tools
let input = InputObject::new(vec![
    Message::user("What's the weather in Tokyo?")
])
.with_tools(vec![Tool::function(function)]);

let response = client.send("gpt-4o", input).await?;

// Handle tool calls
if let Some(tool_calls) = response.tool_calls() {
    for call in tool_calls {
        println!("Function: {}", call.function.name);
        println!("Arguments: {}", call.function.arguments);
    }
}
```

## API Reference

### Client

#### `Edgee::new(config: EdgeeConfig) -> Self`

Create a new client with the given configuration.

#### `Edgee::from_env() -> Result<Self>`

Create a client from environment variables (`EDGEE_API_KEY`, `EDGEE_BASE_URL`).

#### `Edgee::with_api_key(api_key: impl Into<String>) -> Self`

Create a client with just an API key (uses default base URL).

#### `Edgee::send(model: impl Into<String>, input: impl Into<Input>) -> Result<SendResponse>`

Send a non-streaming chat completion request.

- **model**: Model identifier (e.g., "gpt-4o", "mistral-large-latest")
- **input**: Can be a `&str`, `String`, `Vec<Message>`, or `InputObject`

#### `Edgee::stream(model: impl Into<String>, input: impl Into<Input>) -> Result<impl Stream<Item = Result<StreamChunk>>>`

Send a streaming chat completion request.

Returns a `Stream` of `StreamChunk` items that can be processed as they arrive.

### Data Models

#### `Message`

Represents a message in the conversation.

**Constructors:**
- `Message::system(content)` - System message
- `Message::user(content)` - User message
- `Message::assistant(content)` - Assistant message
- `Message::tool(tool_call_id, content)` - Tool response message

#### `InputObject`

Structured input for chat completions.

```rust
let input = InputObject::new(messages)
    .with_tools(tools)
    .with_tool_choice(choice);
```

#### `SendResponse`

Response from a non-streaming request.

**Convenience methods:**
- `text()` - Get text from the first choice
- `message()` - Get the message from the first choice
- `finish_reason()` - Get the finish reason
- `tool_calls()` - Get tool calls from the first choice

#### `StreamChunk`

Chunk from a streaming response.

**Convenience methods:**
- `text()` - Get text delta from the first choice
- `role()` - Get the role from the first choice
- `finish_reason()` - Get the finish reason

### Error Handling

The SDK uses a custom `Error` enum with `thiserror`:

```rust
use edgee::{Edgee, Error};

match client.send("gpt-4o", "Hello").await {
    Ok(response) => println!("{}", response.text().unwrap_or("")),
    Err(Error::Api { status, message }) => {
        eprintln!("API error {}: {}", status, message);
    }
    Err(Error::MissingApiKey) => {
        eprintln!("API key not found");
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

## Supported Models

The SDK works with any model supported by the Edgee AI Gateway, including:

- OpenAI: `gpt-4o`, `gpt-4-turbo`, `gpt-3.5-turbo`
- Anthropic: `claude-3-5-sonnet-20241022`, `claude-3-opus-20240229`
- Mistral: `mistral-large-latest`, `mistral-medium-latest`
- And more...

## Examples

Run the examples to see the SDK in action:

```bash
# Set your API key
export EDGEE_API_KEY="your-api-key"

# Simple example
cargo run --example simple

# Streaming example
cargo run --example streaming

# Tool calling example
cargo run --example tools
```

## Comparison with Python SDK

This Rust SDK provides similar functionality to the Python SDK with Rust-specific improvements:

| Feature | Python SDK | Rust SDK |
|---------|-----------|----------|
| API | Synchronous | Async/await |
| Type Safety | Runtime (dataclasses) | Compile-time (strong types) |
| Error Handling | Exceptions | `Result<T, E>` |
| Streaming | Generator | `Stream` trait |
| Input Flexibility | ✅ | ✅ (with `Into` traits) |
| Tool Calling | ✅ | ✅ |
| Dependencies | Zero (stdlib only) | Minimal (tokio, reqwest, serde) |
| Performance | Good | Excellent (zero-cost abstractions) |

## Rust Idioms Used

This SDK follows Rust best practices:

- **Strong Typing**: Uses enums for roles, structs for messages
- **Builder Pattern**: `EdgeeConfig::new().with_base_url()`
- **Into Traits**: Flexible input with `impl Into<Input>`
- **Error Handling**: `Result<T, E>` with `thiserror`
- **Async/Await**: Non-blocking I/O with tokio
- **Stream Trait**: Idiomatic streaming with `futures::Stream`
- **Option Types**: `Option<T>` for optional fields
- **Zero-Copy**: Efficient string handling with references

## Testing

Run the test suite:

```bash
cargo test
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

## Links

- [Edgee AI Gateway](https://edgee.ai)
- [Documentation](https://docs.rs/edgee)
- [GitHub Repository](https://github.com/edgee-cloud/edgee)
