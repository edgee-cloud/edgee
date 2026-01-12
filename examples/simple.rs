//! Simple example demonstrating basic usage of the Edgee SDK

use edgee::{Edgee, InputObject, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client from environment variables (EDGEE_API_KEY)
    let client = Edgee::from_env()?;

    println!("=== Simple Text Input ===");
    let response = client.send("gpt-4o", "Say 'Hello, Rust!'").await?;
    println!("Response: {}\n", response.text().unwrap_or(""));

    println!("=== Multi-turn Conversation ===");
    let messages = vec![
        Message::system("You are a helpful assistant that speaks like a pirate."),
        Message::user("What's your name?"),
    ];

    let response = client.send("gpt-4o", messages).await?;
    println!("Assistant: {}\n", response.text().unwrap_or(""));

    println!("=== Using InputObject ===");
    let input = InputObject::new(vec![
        Message::system("You are a helpful coding assistant."),
        Message::user("Write a hello world in Rust"),
    ]);

    let response = client.send("gpt-4o", input).await?;
    println!("Assistant: {}\n", response.text().unwrap_or(""));

    println!("=== Response Metadata ===");
    let response = client.send("gpt-4o", "Count to 5").await?;
    println!("Model: {}", response.model);
    println!("Finish Reason: {:?}", response.finish_reason());
    if let Some(usage) = &response.usage {
        println!(
            "Token Usage: {} prompt + {} completion = {} total",
            usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
        );
    }
    println!("Response: {}\n", response.text().unwrap_or(""));

    Ok(())
}
