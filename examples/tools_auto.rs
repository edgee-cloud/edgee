//! Auto tool execution example
//!
//! This example shows how to:
//! - Define tools using the `tool!` macro
//! - Let the SDK automatically execute tools when the model calls them
//! - Get the final response after all tool calls are processed
//!
//! The SDK handles the agentic loop automatically: when the model requests
//! a tool call, the SDK executes your handler and sends the result back
//! to the model until a final response is generated.
//!
//! Run with: cargo run --example tools_auto

use edgee::{tool, Edgee, EdgeeConfig, SimpleInput};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the Edgee client
    let client = Edgee::new(EdgeeConfig::new("your-api-key"));

    // Define a tool using the `tool!` macro
    // The macro creates an ExecutableTool with:
    // - name: the function name the model will call
    // - description: helps the model understand when to use this tool
    // - parameters: JSON schema for the function arguments
    // - handler: async function that executes when the tool is called
    let get_weather = tool!(
        "get_weather",
        "Get the current weather for a location",
        {
            "location" => {"type": "string", "description": "The city name"}
        },
        required: ["location"],
        |args| async move {
            // This handler is called automatically when the model uses this tool
            let location = args["location"].as_str().unwrap_or("Unknown");

            // In a real app, you would call an actual weather API here
            println!("[Tool executed: get_weather for {}]", location);

            json!({
                "location": location,
                "temperature": 22,
                "unit": "celsius",
                "condition": "sunny"
            })
        }
    );

    // Create a SimpleInput with your prompt and tools
    // The SDK will automatically handle tool execution
    let input = SimpleInput::new("What's the weather in Paris?", vec![get_weather]);

    println!("Sending request with auto tool execution...\n");

    // Send the request - the SDK handles the agentic loop automatically
    let response = client.send("devstral2", input).await?;

    // Print the final response (after all tools have been executed)
    println!(
        "\nFinal response: {}",
        response.text().unwrap_or("No response")
    );

    Ok(())
}
