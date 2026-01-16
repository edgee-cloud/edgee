//! Streaming example demonstrating real-time response processing
//!
//! This example shows how to:
//! - Stream responses from a model in real-time
//! - Process chunks as they arrive
//! - Display text progressively (like a typing effect)
//!
//! Run with: cargo run --example streaming

use edgee::{Edgee, EdgeeConfig};
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the Edgee client
    let client = Edgee::new(EdgeeConfig::new("your-api-key"));

    println!("Asking the model to count from 1 to 10...\n");

    // Start a streaming request
    let mut stream = client.stream("devstral2", "Count from 1 to 10").await?;

    // Process each chunk as it arrives
    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                // Print each text chunk as it arrives (no newline, for typing effect)
                if let Some(text) = chunk.text() {
                    print!("{}", text);
                    std::io::Write::flush(&mut std::io::stdout())?;
                }

                // Check if the stream is complete
                if let Some(reason) = chunk.finish_reason() {
                    println!("\n\n[Stream finished: {}]", reason);
                }
            }
            Err(e) => {
                eprintln!("\nError during streaming: {}", e);
                break;
            }
        }
    }

    Ok(())
}
