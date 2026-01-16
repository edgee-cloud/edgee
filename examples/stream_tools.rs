//! Streaming with automatic tool execution example

use edgee::{tool, Edgee, EdgeeConfig, StreamEvent};
use serde_json::json;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Edgee::new(EdgeeConfig::new("your-api-key"));

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

    let mut stream = client
        .stream_with_tools(
            "devstral2",
            "What's the weather in Paris?",
            vec![get_weather],
        )
        .execute()
        .await?;

    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::Chunk(chunk) => {
                if let Some(text) = chunk.text() {
                    print!("{}", text);
                    std::io::Write::flush(&mut std::io::stdout())?;
                }
            }
            StreamEvent::ToolStart { tool_call } => {
                println!("\n[Tool: {}]", tool_call.function.name);
            }
            StreamEvent::ToolResult {
                tool_name, result, ..
            } => {
                println!("[Result: {} -> {}]", tool_name, result);
            }
            StreamEvent::IterationComplete { .. } => {}
        }
    }

    println!();
    Ok(())
}
