//! Manual tool calling example (without auto-execution)

use edgee::{Edgee, EdgeeConfig, FunctionDefinition, InputObject, JsonSchema, Message, Tool};
use serde_json::json;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Edgee::new(EdgeeConfig::new("your-api-key"));

    // Define a weather function
    let get_weather = FunctionDefinition {
        name: "get_weather".to_string(),
        description: Some("Get the current weather for a location".to_string()),
        parameters: JsonSchema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = HashMap::new();
                props.insert("location".to_string(), json!({"type": "string"}));
                props
            }),
            required: Some(vec!["location".to_string()]),
            description: None,
        },
    };

    // Send request with tool
    let input = InputObject::new(vec![Message::user("What's the weather in Paris?")])
        .with_tools(vec![Tool::function(get_weather)]);

    let response = client.send("devstral2", input).await?;

    // Handle tool calls manually
    if let Some(tool_calls) = response.tool_calls() {
        for call in tool_calls {
            println!("Tool called: {}", call.function.name);
            println!("Arguments: {}", call.function.arguments);
        }
    }

    Ok(())
}
