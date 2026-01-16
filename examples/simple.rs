//! Simple example demonstrating basic usage of the Edgee SDK

use edgee::{Edgee, EdgeeConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Edgee::new(EdgeeConfig::new("your-api-key"));

    let response = client.send("devstral2", "Say 'Hello, Rust!'").await?;
    println!("Response: {}", response.text().unwrap_or(""));

    Ok(())
}
