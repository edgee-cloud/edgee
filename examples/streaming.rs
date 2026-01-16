//! Streaming example demonstrating real-time response processing

use edgee::{Edgee, EdgeeConfig};
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Edgee::new(
        EdgeeConfig::new("your-api-key")
    );

    let mut stream = client.stream("devstral2", "Count from 1 to 10").await?;

    while let Some(result) = stream.next().await {
        if let Ok(chunk) = result {
            if let Some(text) = chunk.text() {
                print!("{}", text);
                std::io::Write::flush(&mut std::io::stdout())?;
            }
        }
    }

    println!();
    Ok(())
}
