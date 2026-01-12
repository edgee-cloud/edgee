//! Streaming example demonstrating real-time response processing

use edgee::{Edgee, Message};
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Edgee::from_env()?;

    println!("=== Simple Streaming ===");
    println!("Streaming response: ");

    let mut stream = client.stream("gpt-4o", "Count from 1 to 10 slowly").await?;

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                if let Some(text) = chunk.text() {
                    print!("{}", text);
                    std::io::Write::flush(&mut std::io::stdout())?;
                }

                if let Some(reason) = chunk.finish_reason() {
                    println!("\n[Finish reason: {}]", reason);
                }
            }
            Err(e) => {
                eprintln!("\nError: {}", e);
                break;
            }
        }
    }

    println!("\n");

    println!("=== Streaming with System Message ===");
    println!("Streaming response: ");

    let messages = vec![
        Message::system("You are a poetic assistant. Respond in haiku format."),
        Message::user("Describe Rust programming language"),
    ];

    let mut stream = client.stream("gpt-4o", messages).await?;

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                if let Some(text) = chunk.text() {
                    print!("{}", text);
                    std::io::Write::flush(&mut std::io::stdout())?;
                }
            }
            Err(e) => {
                eprintln!("\nError: {}", e);
                break;
            }
        }
    }

    println!("\n");

    println!("=== Collecting Full Response from Stream ===");
    let mut stream = client.stream("gpt-4o", "Say hello in 5 languages").await?;

    let mut full_text = String::new();
    while let Some(result) = stream.next().await {
        if let Ok(chunk) = result {
            if let Some(text) = chunk.text() {
                full_text.push_str(text);
            }
        }
    }

    println!("Full response: {}", full_text);

    Ok(())
}
