//! # Edgee Rust SDK
//!
//! A Rust SDK for the [Edgee AI Gateway](https://www.edgee.ai).
//!
//! This SDK provides a simple, idiomatic Rust interface for interacting with the Edgee AI Gateway,
//! which supports multiple LLM providers including OpenAI, Anthropic, Mistral, and more.
//!
//! ## Features
//!
//! - **Async/await support** - Built on tokio for efficient async operations
//! - **Type-safe** - Strong typing with Rust enums and structs
//! - **Streaming** - Full support for streaming responses
//! - **Tool calling** - Support for function/tool calling
//! - **Flexible input** - Accept strings, message arrays, or structured objects
//! - **Error handling** - Comprehensive error types with `thiserror`
//! - **Zero-cost abstractions** - Efficient implementation with minimal overhead
//!
//! ## Quick Start
//!
//! ```no_run
//! use edgee::Edgee;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create client from environment variables (EDGEE_API_KEY)
//!     let client = Edgee::from_env()?;
//!
//!     // Simple text completion
//!     let response = client.send("gpt-4o", "Hello, world!").await?;
//!     println!("{}", response.text().unwrap_or(""));
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Streaming Example
//!
//! ```no_run
//! use edgee::Edgee;
//! use tokio_stream::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = Edgee::from_env()?;
//!
//!     let mut stream = client.stream("gpt-4o", "Tell me a story").await?;
//!
//!     while let Some(chunk) = stream.next().await {
//!         if let Ok(chunk) = chunk {
//!             if let Some(text) = chunk.text() {
//!                 print!("{}", text);
//!             }
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Tool Calling Example
//!
//! ```no_run
//! use edgee::{Edgee, Message, InputObject, Tool, FunctionDefinition, JsonSchema};
//! use std::collections::HashMap;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = Edgee::from_env()?;
//!
//!     // Define a function
//!     let function = FunctionDefinition {
//!         name: "get_weather".to_string(),
//!         description: Some("Get the weather for a location".to_string()),
//!         parameters: JsonSchema {
//!             schema_type: "object".to_string(),
//!             properties: Some({
//!                 let mut props = HashMap::new();
//!                 props.insert("location".to_string(), serde_json::json!({
//!                     "type": "string",
//!                     "description": "The city and state, e.g. San Francisco, CA"
//!                 }));
//!                 props
//!             }),
//!             required: Some(vec!["location".to_string()]),
//!             description: None,
//!         },
//!     };
//!
//!     let input = InputObject::new(vec![
//!         Message::user("What's the weather in San Francisco?")
//!     ])
//!     .with_tools(vec![Tool::function(function)]);
//!
//!     let response = client.send("gpt-4o", input).await?;
//!
//!     if let Some(tool_calls) = response.tool_calls() {
//!         println!("Tool calls: {:?}", tool_calls);
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod error;
pub mod models;

// Re-export main types for convenience
pub use client::{Edgee, Input};
pub use error::{Error, Result};
pub use models::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exports() {
        // Verify that all main types are accessible
        let _: Result<()> = Ok(());
        let _config = EdgeeConfig::new("test");
        let _msg = Message::user("test");
    }
}
