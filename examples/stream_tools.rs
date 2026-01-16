//! Streaming with automatic tool execution example
//!
//! This example shows how to:
//! - Combine streaming with automatic tool execution
//! - Receive real-time events for chunks, tool calls, and results
//! - Display the response progressively while tools are being executed
//!
//! This is useful when you want to show progress to the user while
//! tools are being executed in the background.
//!
//! Run with: cargo run --example stream_tools

use edgee::{tool, Edgee, EdgeeConfig, StreamEvent};
use serde_json::json;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the Edgee client
    let client = Edgee::new(EdgeeConfig::new("your-api-key"));

    // Define a tool using the `tool!` macro
    let get_weather = tool!(
        "get_weather",
        "Get the current weather for a location",
        {
            "location" => {"type": "string", "description": "The city name"}
        },
        required: ["location"],
        |args| async move {
            let location = args["location"].as_str().unwrap_or("Unknown");

            // Simulate an API call
            json!({
                "location": location,
                "temperature": 22,
                "unit": "celsius",
                "condition": "sunny"
            })
        }
    );

    println!("Streaming request with auto tool execution...\n");

    // Start a streaming request with tools
    let mut stream = client
        .stream_with_tools("devstral2", "What's the weather in Paris?", vec![get_weather])
        .execute()
        .await?;

    // Process events as they arrive
    while let Some(event) = stream.next().await {
        match event? {
            // Text chunks from the model
            StreamEvent::Chunk(chunk) => {
                if let Some(text) = chunk.text() {
                    print!("{}", text);
                    std::io::Write::flush(&mut std::io::stdout())?;
                }
            }

            // A tool is about to be executed
            StreamEvent::ToolStart { tool_call } => {
                println!("\n[Calling tool: {}]", tool_call.function.name);
            }

            // A tool has finished executing
            StreamEvent::ToolResult {
                tool_name, result, ..
            } => {
                println!("[Tool result: {} returned {}]", tool_name, result);
            }

            // An iteration of the agentic loop is complete
            StreamEvent::IterationComplete { iteration } => {
                println!("[Iteration {} complete]", iteration);
            }
        }
    }

    println!("\n\nDone!");

    Ok(())
}
