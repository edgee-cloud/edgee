//! Example: Token compression with Edgee Gateway SDK
//!
//! This example demonstrates how to:
//! 1. Enable compression for a request using the builder pattern
//! 2. Set a custom compression rate
//! 3. Access compression metrics from the response

use edgee::{Edgee, InputObject, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client from environment variables (EDGEE_API_KEY)
    let client = Edgee::from_env()?;

    println!("{}", "=".repeat(70));
    println!("Edgee Token Compression Example");
    println!("{}", "=".repeat(70));
    println!();

    // Example: Request with compression enabled
    println!("Example: Request with compression enabled");
    println!("{}", "-".repeat(70));

    // Create input with compression settings using builder pattern
    let input = InputObject::new(vec![Message::user(
        "Explain quantum computing in simple terms.",
    )])
    .with_compression(true)
    .with_compression_rate(0.5);

    let response = client.send("gpt-4o", input).await?;

    println!("Response: {}", response.text().unwrap_or(""));
    println!();

    // Display usage information
    if let Some(usage) = &response.usage {
        println!("Token Usage:");
        println!("  Prompt tokens:     {}", usage.prompt_tokens);
        println!("  Completion tokens: {}", usage.completion_tokens);
        println!("  Total tokens:      {}", usage.total_tokens);
        println!();
    }

    // Display compression information
    if let Some(compression) = &response.compression {
        println!("Compression Metrics:");
        println!("  Input tokens:  {}", compression.input_tokens);
        println!("  Saved tokens:  {}", compression.saved_tokens);
        println!("  Compression rate: {:.2}%", compression.rate * 100.0);
        println!("  Token savings: {} tokens saved!", compression.saved_tokens);
    } else {
        println!("No compression data available in response.");
        println!("Note: Compression data is only returned when compression is enabled");
        println!("      and supported by your API key configuration.");
    }

    println!();
    println!("{}", "=".repeat(70));

    Ok(())
}
