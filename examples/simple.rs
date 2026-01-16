//! Simple example demonstrating basic usage of the Edgee SDK
//!
//! This example shows how to:
//! - Create an Edgee client with your API key
//! - Send a simple text prompt to a model
//! - Get the response text
//!
//! Run with: cargo run --example simple

use edgee::{Edgee, EdgeeConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the Edgee client with your API key
    let client = Edgee::new(EdgeeConfig::new("your-api-key"));

    // Send a simple text prompt to the model
    let response = client
        .send("devstral2", "What is the capital of France?")
        .await?;

    // Print the response text
    println!("Response: {}", response.text().unwrap_or("No response"));

    // You can also access metadata about the response
    println!("Model used: {}", response.model);

    if let Some(usage) = &response.usage {
        println!(
            "Tokens: {} prompt + {} completion = {} total",
            usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
        );
    }

    Ok(())
}
