//! Example: Token compression with Edgee Gateway SDK
//!
//! This example demonstrates how to:
//! 1. Enable compression for a request with a large input context using the builder pattern
//! 2. Set a custom compression rate
//! 3. Access compression metrics from the response
//!
//! IMPORTANT: Only USER messages are compressed. System messages are not compressed.
//! This example includes a large context in the user message to demonstrate meaningful
//! compression savings.

use edgee::{Edgee, InputObject, Message};

// Large context document to demonstrate input compression
const LARGE_CONTEXT: &str = r#"
The History and Impact of Artificial Intelligence

Artificial intelligence (AI) has evolved from a theoretical concept to a 
transformative technology that influences nearly every aspect of modern life. 
The field began in earnest in the 1950s when pioneers like Alan Turing and 
John McCarthy laid the groundwork for machine intelligence.

Early developments focused on symbolic reasoning and expert systems. These 
rule-based approaches dominated the field through the 1970s and 1980s, with 
systems like MYCIN demonstrating practical applications in medical diagnosis. 
However, these early systems were limited by their inability to learn from data 
and adapt to new situations.

The resurgence of neural networks in the 1980s and 1990s, particularly with 
backpropagation algorithms, opened new possibilities. Yet it wasn't until the 
2010s, with the advent of deep learning and the availability of massive datasets 
and computational power, that AI truly began to revolutionize industries.

Modern AI applications span numerous domains:
- Natural language processing enables machines to understand and generate human language
- Computer vision allows machines to interpret visual information from the world
- Robotics combines AI with mechanical systems for autonomous operation
- Healthcare uses AI for diagnosis, drug discovery, and personalized treatment
- Finance leverages AI for fraud detection, algorithmic trading, and risk assessment
- Transportation is being transformed by autonomous vehicles and traffic optimization

The development of large language models like GPT, BERT, and others has 
particularly accelerated progress in natural language understanding and generation. 
These models, trained on vast amounts of text data, can perform a wide range of 
language tasks with remarkable proficiency.

Despite remarkable progress, significant challenges remain. Issues of bias, 
interpretability, safety, and ethical considerations continue to be areas of 
active research and debate. The AI community is working to ensure that these 
powerful technologies are developed and deployed responsibly, with consideration 
for their societal impact.

Looking forward, AI is expected to continue advancing rapidly, with potential 
breakthroughs in areas like artificial general intelligence, quantum machine 
learning, and brain-computer interfaces. The integration of AI into daily life 
will likely deepen, raising important questions about human-AI collaboration, 
workforce transformation, and the future of human cognition itself.
"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client from environment variables (EDGEE_API_KEY)
    let client = Edgee::from_env()?;

    println!("{}", "=".repeat(70));
    println!("Edgee Token Compression Example");
    println!("{}", "=".repeat(70));
    println!();

    // Example: Request with compression enabled and large input
    println!("Example: Large user message with compression enabled");
    println!("{}", "-".repeat(70));
    println!("Input context length: {} characters", LARGE_CONTEXT.len());
    println!();

    // NOTE: Only USER messages are compressed
    // Put the large context in the user message to demonstrate compression
    let user_message = format!(
        "Here is some context about AI:\n\n{}\n\nBased on this context, summarize the key milestones in AI development in 3 bullet points.",
        LARGE_CONTEXT
    );

    // Create input with compression settings using builder pattern
    let input = InputObject::new(vec![Message::user(user_message)])
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

        let savings_pct = if compression.input_tokens > 0 {
            (compression.saved_tokens as f64 / compression.input_tokens as f64) * 100.0
        } else {
            0.0
        };
        println!("  Savings: {:.1}% of input tokens saved!", savings_pct);
        println!();
        println!("  💡 Without compression, this request would have used");
        println!("     {} input tokens.", compression.input_tokens);
        println!(
            "     With compression, only {} tokens were processed!",
            compression.input_tokens - compression.saved_tokens
        );
    } else {
        println!("No compression data available in response.");
        println!("Note: Compression data is only returned when compression is enabled");
        println!("      and supported by your API key configuration.");
    }

    println!();
    println!("{}", "=".repeat(70));

    Ok(())
}
