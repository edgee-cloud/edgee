//! Auto tool execution example with automatic tool calling

use edgee::{tool, Edgee, EdgeeConfig, SimpleInput};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Edgee::new(
        EdgeeConfig::new("your-api-key")
    );

    let get_weather = tool!(
        "get_weather",
        "Get the current weather for a location",
        {
            "location" => {"type": "string", "description": "The city name"}
        },
        required: ["location"],
        |args| async move {
            let location = args["location"].as_str().unwrap_or("Unknown");
            json!({
                "location": location,
                "temperature": 22,
                "condition": "sunny"
            })
        }
    );

    let input = SimpleInput::new(
        "What's the weather in Paris?",
        vec![get_weather],
    );

    let response = client.send("devstral2", input).await?;
    println!("Response: {}", response.text().unwrap_or(""));

    Ok(())
}
