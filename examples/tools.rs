//! Manual tool calling example
//!
//! This example shows how to:
//! - Define tools manually (without the `tool!` macro)
//! - Send a request with tools
//! - Handle tool calls yourself (manual execution)
//!
//! Use this approach when you need full control over tool execution,
//! such as when tools require user confirmation or have side effects.
//!
//! For automatic tool execution, see the `tools_auto` example instead.
//!
//! Run with: cargo run --example tools

use edgee::{Edgee, EdgeeConfig, FunctionDefinition, InputObject, JsonSchema, Message, Tool};
use serde_json::json;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the Edgee client
    let client = Edgee::new(EdgeeConfig::new("your-api-key"));

    // Define a tool manually using FunctionDefinition
    // This gives you full control over the tool schema
    let get_weather = FunctionDefinition {
        name: "get_weather".to_string(),
        description: Some("Get the current weather for a location".to_string()),
        parameters: JsonSchema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = HashMap::new();
                props.insert(
                    "location".to_string(),
                    json!({
                        "type": "string",
                        "description": "The city name, e.g. Paris"
                    }),
                );
                props
            }),
            required: Some(vec!["location".to_string()]),
            description: None,
        },
    };

    // Create an input with messages and tools
    let input = InputObject::new(vec![Message::user("What's the weather in Paris?")])
        .with_tools(vec![Tool::function(get_weather)]);

    println!("Sending request with tools...\n");

    // Send the request
    let response = client.send("devstral2", input).await?;

    // Check if the model requested any tool calls
    if let Some(tool_calls) = response.tool_calls() {
        println!("Model requested {} tool call(s):\n", tool_calls.len());

        for call in tool_calls {
            println!("  Tool: {}", call.function.name);
            println!("  Arguments: {}", call.function.arguments);
            println!("  Call ID: {}", call.id);
            println!();

            // Here you would:
            // 1. Execute the tool with the provided arguments
            // 2. Create a new request with the tool result
            // 3. Send it back to the model for a final response
            //
            // Example:
            // let result = execute_my_tool(&call.function.name, &call.function.arguments);
            // let tool_message = Message::tool(call.id.clone(), result);
            // ... send another request with the tool message
        }
    } else {
        // No tool calls - the model responded directly
        println!("Response: {}", response.text().unwrap_or("No response"));
    }

    Ok(())
}
