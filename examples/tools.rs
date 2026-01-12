//! Tool calling example demonstrating function calling capabilities

use edgee::{Edgee, FunctionDefinition, InputObject, JsonSchema, Message, Tool};
use serde_json::json;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Edgee::from_env()?;

    println!("=== Tool Calling Example ===\n");

    // Define a weather function
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
                        "description": "The city and state, e.g. San Francisco, CA"
                    }),
                );
                props.insert(
                    "unit".to_string(),
                    json!({
                        "type": "string",
                        "enum": ["celsius", "fahrenheit"],
                        "description": "The temperature unit"
                    }),
                );
                props
            }),
            required: Some(vec!["location".to_string()]),
            description: None,
        },
    };

    // Define a calculator function
    let calculate = FunctionDefinition {
        name: "calculate".to_string(),
        description: Some("Perform a mathematical calculation".to_string()),
        parameters: JsonSchema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = HashMap::new();
                props.insert(
                    "operation".to_string(),
                    json!({
                        "type": "string",
                        "enum": ["add", "subtract", "multiply", "divide"],
                        "description": "The operation to perform"
                    }),
                );
                props.insert(
                    "a".to_string(),
                    json!({
                        "type": "number",
                        "description": "First operand"
                    }),
                );
                props.insert(
                    "b".to_string(),
                    json!({
                        "type": "number",
                        "description": "Second operand"
                    }),
                );
                props
            }),
            required: Some(vec![
                "operation".to_string(),
                "a".to_string(),
                "b".to_string(),
            ]),
            description: None,
        },
    };

    // Create input with tools
    let input = InputObject::new(vec![Message::user(
        "What's the weather in San Francisco? Also, what's 15 multiplied by 7?",
    )])
    .with_tools(vec![Tool::function(get_weather), Tool::function(calculate)]);

    println!("Sending request with tools...\n");
    let response = client.send("gpt-4o", input).await?;

    // Check if the model made tool calls
    if let Some(tool_calls) = response.tool_calls() {
        println!("Model made {} tool call(s):\n", tool_calls.len());

        for (i, call) in tool_calls.iter().enumerate() {
            println!("Tool Call {}:", i + 1);
            println!("  ID: {}", call.id);
            println!("  Function: {}", call.function.name);
            println!("  Arguments: {}", call.function.arguments);
            println!();

            // Simulate executing the function
            let result = match call.function.name.as_str() {
                "get_weather" => {
                    let args: serde_json::Value = serde_json::from_str(&call.function.arguments)?;
                    let location = args["location"].as_str().unwrap_or("Unknown");
                    format!("The weather in {} is sunny, 72°F", location)
                }
                "calculate" => {
                    let args: serde_json::Value = serde_json::from_str(&call.function.arguments)?;
                    let op = args["operation"].as_str().unwrap_or("add");
                    let a = args["a"].as_f64().unwrap_or(0.0);
                    let b = args["b"].as_f64().unwrap_or(0.0);

                    let result = match op {
                        "add" => a + b,
                        "subtract" => a - b,
                        "multiply" => a * b,
                        "divide" => {
                            if b != 0.0 {
                                a / b
                            } else {
                                return Err("Division by zero".into());
                            }
                        }
                        _ => 0.0,
                    };

                    format!("The result is {}", result)
                }
                _ => "Unknown function".to_string(),
            };

            println!("  Result: {}\n", result);
        }

        // Send tool results back to get final answer
        println!("Sending tool results back to model...\n");

        let mut messages = vec![Message::user(
            "What's the weather in San Francisco? Also, what's 15 multiplied by 7?",
        )];

        // Add the assistant's response with tool calls
        if let Some(first_choice) = response.choices.first() {
            messages.push(first_choice.message.clone());
        }

        // Add tool responses
        for call in tool_calls {
            let result = match call.function.name.as_str() {
                "get_weather" => "The weather in San Francisco is sunny, 72°F".to_string(),
                "calculate" => "The result is 105".to_string(),
                _ => "Unknown function".to_string(),
            };

            messages.push(Message::tool(call.id.clone(), result));
        }

        let final_input = InputObject::new(messages);
        let final_response = client.send("gpt-4o", final_input).await?;

        println!("Final response:");
        println!("{}\n", final_response.text().unwrap_or(""));
    } else {
        println!("No tool calls made. Response:");
        println!("{}\n", response.text().unwrap_or(""));
    }

    Ok(())
}
