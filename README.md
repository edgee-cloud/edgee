# Edgee Rust SDK

Modern, type-safe Rust SDK for the [Edgee AI Gateway](https://www.edgee.cloud).

[![Crates.io](https://img.shields.io/crates/v/edgee.svg)](https://crates.io/crates/edgee)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
edgee = "2.0"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

```rust
use edgee::Edgee;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Edgee::from_env()?;

    let response = client.send("gpt-4o", "What is the capital of France?").await?;
    println!("{}", response.text().unwrap_or(""));
    // "The capital of France is Paris."

    Ok(())
}
```

## Send Method

The `send()` method makes non-streaming chat completion requests:

```rust
let response = client.send("gpt-4o", "Hello, world!").await?;

// Access response
println!("{}", response.text().unwrap_or(""));      // Text content
println!("{:?}", response.finish_reason());         // Finish reason
if let Some(tool_calls) = response.tool_calls() {    // Tool calls (if any)
    println!("{:?}", tool_calls);
}
```

## Stream Method

The `stream()` method enables real-time streaming responses:

```rust
use tokio_stream::StreamExt;

let mut stream = client.stream("gpt-4o", "Tell me a story").await?;

while let Some(result) = stream.next().await {
    match result {
        Ok(chunk) => {
            if let Some(text) = chunk.text() {
                print!("{}", text);
            }
            
            if let Some(reason) = chunk.finish_reason() {
                println!("\nFinished: {}", reason);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Features

- ✅ **Type-safe** - Leverages Rust's powerful type system
- ✅ **Async/await** - Built on tokio for efficient async operations
- ✅ **OpenAI-compatible** - Works with any model supported by Edgee
- ✅ **Streaming** - First-class support with `Stream` trait
- ✅ **Tool calling** - Full support for function calling
- ✅ **Zero-cost abstractions** - Efficient implementation with minimal overhead

## Documentation

For complete documentation, examples, and API reference, visit:

**👉 [Official Rust SDK Documentation](https://www.edgee.cloud/docs/sdk/rust)**

The documentation includes:
- [Configuration guide](https://www.edgee.cloud/docs/sdk/rust/configuration) - Multiple ways to configure the SDK
- [Send method](https://www.edgee.cloud/docs/sdk/rust/send) - Complete guide to non-streaming requests
- [Stream method](https://www.edgee.cloud/docs/sdk/rust/stream) - Streaming responses guide
- [Tools](https://www.edgee.cloud/docs/sdk/rust/tools) - Function calling guide

## Examples

Run the examples to see the SDK in action:

```bash
export EDGEE_API_KEY="your-api-key"
cargo run --example simple
cargo run --example streaming
cargo run --example tools
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
