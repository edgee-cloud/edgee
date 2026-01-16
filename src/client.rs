use crate::{
    error::{Error, Result},
    models::*,
};
use async_stream::try_stream;
use bytes::Bytes;
use futures::stream::{Stream, StreamExt};
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;
use std::pin::Pin;

/// Input types accepted by the send method
#[derive(Debug, Clone)]
pub enum Input {
    /// Simple text input (converted to a user message)
    Text(String),
    /// Structured input with messages and tools (advanced mode - manual tool handling)
    Object(InputObject),
    /// Simple text input with executable tools (simple mode - auto tool execution)
    SimpleWithTools(SimpleInput),
}

/// Simple mode input with executable tools
///
/// Used for automatic tool execution where the SDK handles the agentic loop.
#[derive(Debug, Clone)]
pub struct SimpleInput {
    /// The user's text input
    pub text: String,
    /// Executable tools with handlers
    pub tools: Vec<ExecutableTool>,
    /// Maximum number of tool execution iterations (default: 10)
    pub max_iterations: u32,
}

impl SimpleInput {
    /// Create a new simple input with tools
    pub fn new(text: impl Into<String>, tools: Vec<ExecutableTool>) -> Self {
        Self {
            text: text.into(),
            tools,
            max_iterations: 10,
        }
    }

    /// Set the maximum number of tool iterations
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }
}

impl From<String> for Input {
    fn from(s: String) -> Self {
        Input::Text(s)
    }
}

impl From<&str> for Input {
    fn from(s: &str) -> Self {
        Input::Text(s.to_string())
    }
}

impl From<InputObject> for Input {
    fn from(obj: InputObject) -> Self {
        Input::Object(obj)
    }
}

impl From<Vec<Message>> for Input {
    fn from(messages: Vec<Message>) -> Self {
        Input::Object(InputObject::new(messages))
    }
}

impl From<SimpleInput> for Input {
    fn from(input: SimpleInput) -> Self {
        Input::SimpleWithTools(input)
    }
}

/// Main client for interacting with the Edgee AI Gateway
#[derive(Debug, Clone)]
pub struct Edgee {
    config: EdgeeConfig,
    client: Client,
}

impl Edgee {
    /// Create a new Edgee client with the given configuration
    pub fn new(config: EdgeeConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    /// Create a new Edgee client from environment variables
    /// Reads EDGEE_API_KEY and optionally EDGEE_BASE_URL
    pub fn from_env() -> Result<Self> {
        let config = EdgeeConfig::from_env()?;
        Ok(Self::new(config))
    }

    /// Create a new Edgee client with just an API key (uses default base URL)
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        Self::new(EdgeeConfig::new(api_key))
    }

    /// Send a chat completion request (non-streaming)
    ///
    /// This method is polymorphic and handles different input types:
    /// - **String/&str**: Simple text input (converted to a user message)
    /// - **Vec<Message>**: Multi-turn conversation
    /// - **InputObject**: Advanced mode with manual tool handling
    /// - **SimpleInput**: Simple mode with automatic tool execution
    ///
    /// # Arguments
    /// * `model` - The model to use (e.g., "gpt-4o", "mistral-large-latest")
    /// * `input` - The input (polymorphic - see examples below)
    ///
    /// # Examples
    ///
    /// Simple text:
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use edgee::Edgee;
    ///
    /// let client = Edgee::from_env()?;
    /// let response = client.send("gpt-4o", "Hello, world!").await?;
    /// println!("{}", response.text().unwrap_or(""));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// With auto-executed tools (simple mode):
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use edgee::{Edgee, SimpleInput, ExecutableTool, JsonSchema};
    /// use serde_json::json;
    /// use std::collections::HashMap;
    ///
    /// let client = Edgee::from_env()?;
    ///
    /// let weather_tool = ExecutableTool::new(
    ///     "get_weather", "Get weather", JsonSchema {
    ///         schema_type: "object".to_string(),
    ///         properties: None, required: None, description: None,
    ///     },
    ///     |_args| async move { json!({"temp": 72}) },
    /// );
    ///
    /// // Simple mode: tools are auto-executed
    /// let input = SimpleInput::new("What's the weather?", vec![weather_tool]);
    /// let response = client.send("gpt-4o", input).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send(
        &self,
        model: impl Into<String>,
        input: impl Into<Input>,
    ) -> Result<SendResponse> {
        let model = model.into();
        let input = input.into();

        // Polymorphic dispatch based on input type
        match input {
            // Simple mode with auto tool execution
            Input::SimpleWithTools(simple) => {
                self.execute_with_tools(
                    model,
                    simple.text,
                    simple.tools,
                    simple.max_iterations,
                )
                .await
            }
            // Text or Object mode - standard API call
            _ => self.send_standard(model, input).await,
        }
    }

    /// Standard send without auto tool execution
    async fn send_standard(&self, model: String, input: Input) -> Result<SendResponse> {
        let (messages, tools, tool_choice) = self.parse_input(input);

        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": false,
        });

        if let Some(tools) = tools {
            body["tools"] = json!(tools);
        }
        if let Some(tool_choice) = tool_choice {
            body["tool_choice"] = tool_choice;
        }

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::Api { status, message });
        }

        let send_response: SendResponse = response.json().await?;
        Ok(send_response)
    }

    /// Send a chat completion request with streaming
    ///
    /// Returns a stream of chunks that can be processed as they arrive
    ///
    /// # Arguments
    /// * `model` - The model to use (e.g., "gpt-4o", "mistral-large-latest")
    /// * `input` - The input (can be a string, InputObject, or `Vec<Message>`)
    ///
    /// # Example
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use edgee::Edgee;
    /// use tokio_stream::StreamExt;
    ///
    /// let client = Edgee::from_env()?;
    /// let mut stream = client.stream("gpt-4o", "Tell me a story").await?;
    ///
    /// while let Some(chunk) = stream.next().await {
    ///     if let Ok(chunk) = chunk {
    ///         if let Some(text) = chunk.text() {
    ///             print!("{}", text);
    ///         }
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn stream(
        &self,
        model: impl Into<String>,
        input: impl Into<Input>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
        let input = input.into();
        let (messages, tools, tool_choice) = self.parse_input(input);

        let mut body = json!({
            "model": model.into(),
            "messages": messages,
            "stream": true,
        });

        if let Some(tools) = tools {
            body["tools"] = json!(tools);
        }
        if let Some(tool_choice) = tool_choice {
            body["tool_choice"] = tool_choice;
        }

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::Api { status, message });
        }

        let stream = response.bytes_stream();
        let parsed_stream = Self::parse_sse_stream(stream);

        Ok(Box::pin(parsed_stream))
    }

    /// Parse SSE stream into StreamChunk objects
    fn parse_sse_stream(
        stream: impl Stream<Item = reqwest::Result<Bytes>> + Send + 'static,
    ) -> impl Stream<Item = Result<StreamChunk>> + Send {
        let mut buffer = String::new();

        stream
            .map(move |result| {
                let bytes = result.map_err(Error::Http)?;
                let text = String::from_utf8_lossy(&bytes);
                buffer.push_str(&text);

                let mut chunks = Vec::new();
                while let Some(pos) = buffer.find("\n\n") {
                    let chunk = buffer[..pos].to_string();
                    buffer.drain(..pos + 2);

                    if chunk.is_empty() {
                        continue;
                    }

                    // Parse SSE format: "data: {...}"
                    for line in chunk.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                continue;
                            }

                            match serde_json::from_str::<StreamChunk>(data) {
                                Ok(parsed_chunk) => chunks.push(Ok(parsed_chunk)),
                                Err(e) => {
                                    // Skip malformed JSON (similar to Python SDK behavior)
                                    eprintln!("Failed to parse chunk: {}", e);
                                }
                            }
                        }
                    }
                }

                Ok(chunks)
            })
            .flat_map(|result: Result<Vec<Result<StreamChunk>>>| match result {
                Ok(chunks) => futures::stream::iter(chunks).boxed(),
                Err(e) => futures::stream::once(async move { Err(e) }).boxed(),
            })
    }

    /// Parse input into components (for standard mode only)
    fn parse_input(
        &self,
        input: Input,
    ) -> (Vec<Message>, Option<Vec<Tool>>, Option<serde_json::Value>) {
        match input {
            Input::Text(text) => {
                let messages = vec![Message::user(text)];
                (messages, None, None)
            }
            Input::Object(obj) => (obj.messages, obj.tools, obj.tool_choice),
            // SimpleWithTools is handled separately in send() before reaching here
            Input::SimpleWithTools(_) => unreachable!(
                "SimpleWithTools should be handled by execute_with_tools, not parse_input"
            ),
        }
    }

    /// Send a request with executable tools that are automatically called
    ///
    /// This is "simple mode" - the SDK will automatically execute tool calls
    /// and feed the results back to the model in an agentic loop until the
    /// model produces a final response or max iterations is reached.
    ///
    /// # Arguments
    /// * `model` - The model to use (e.g., "gpt-4o", "mistral-large-latest")
    /// * `input` - The text input (converted to a user message)
    /// * `tools` - Executable tools with handlers
    ///
    /// # Example
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use edgee::{Edgee, ExecutableTool, JsonSchema};
    /// use serde_json::json;
    /// use std::collections::HashMap;
    ///
    /// let client = Edgee::from_env()?;
    ///
    /// let weather_tool = ExecutableTool::new(
    ///     "get_weather",
    ///     "Get weather for a location",
    ///     JsonSchema {
    ///         schema_type: "object".to_string(),
    ///         properties: Some({
    ///             let mut props = HashMap::new();
    ///             props.insert("location".to_string(), json!({
    ///                 "type": "string",
    ///                 "description": "The city name"
    ///             }));
    ///             props
    ///         }),
    ///         required: Some(vec!["location".to_string()]),
    ///         description: None,
    ///     },
    ///     |args| async move {
    ///         let location = args["location"].as_str().unwrap_or("Unknown");
    ///         json!({"temperature": 72, "unit": "F", "location": location})
    ///     },
    /// );
    ///
    /// let response = client
    ///     .send_with_tools("gpt-4o", "What's the weather in Paris?", vec![weather_tool])
    ///     .await?;
    ///
    /// println!("{}", response.text().unwrap_or(""));
    /// # Ok(())
    /// # }
    /// ```
    pub fn send_with_tools(
        &self,
        model: impl Into<String>,
        input: impl Into<String>,
        tools: Vec<ExecutableTool>,
    ) -> SendWithToolsBuilder {
        SendWithToolsBuilder::new(self.clone(), model.into(), input.into(), tools)
    }

    /// Stream a request with executable tools that are automatically called
    ///
    /// This combines streaming with automatic tool execution. The SDK streams
    /// the response and automatically executes tool calls, yielding events
    /// for chunks, tool starts, and tool results.
    ///
    /// # Arguments
    /// * `model` - The model to use (e.g., "gpt-4o", "mistral-large-latest")
    /// * `input` - The text input (converted to a user message)
    /// * `tools` - Executable tools with handlers
    ///
    /// # Example
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use edgee::{Edgee, StreamEvent, tool};
    /// use serde_json::json;
    /// use tokio_stream::StreamExt;
    ///
    /// let client = Edgee::from_env()?;
    ///
    /// let weather = tool!(
    ///     "get_weather",
    ///     "Get weather for a location",
    ///     { "location" => {"type": "string"} },
    ///     required: ["location"],
    ///     |args| async move {
    ///         json!({"temperature": 72})
    ///     }
    /// );
    ///
    /// let mut stream = client
    ///     .stream_with_tools("gpt-4o", "What's the weather in Paris?", vec![weather])
    ///     .execute()
    ///     .await?;
    ///
    /// while let Some(event) = stream.next().await {
    ///     match event? {
    ///         StreamEvent::Chunk(chunk) => {
    ///             if let Some(text) = chunk.text() {
    ///                 print!("{}", text);
    ///             }
    ///         }
    ///         StreamEvent::ToolStart { tool_call } => {
    ///             println!("\n[Tool: {}]", tool_call.function.name);
    ///         }
    ///         StreamEvent::ToolResult { tool_name, result, .. } => {
    ///             println!("[Result: {} -> {}]", tool_name, result);
    ///         }
    ///         StreamEvent::IterationComplete { iteration } => {
    ///             println!("[Iteration {} complete]", iteration);
    ///         }
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn stream_with_tools(
        &self,
        model: impl Into<String>,
        input: impl Into<String>,
        tools: Vec<ExecutableTool>,
    ) -> StreamWithToolsBuilder {
        StreamWithToolsBuilder::new(self.clone(), model.into(), input.into(), tools)
    }

    /// Internal method to execute the agentic loop with tools
    async fn execute_with_tools(
        &self,
        model: String,
        input: String,
        tools: Vec<ExecutableTool>,
        max_iterations: u32,
    ) -> Result<SendResponse> {
        // Build initial messages
        let mut messages: Vec<Message> = vec![Message::user(input)];

        // Convert ExecutableTool to API format
        let api_tools: Vec<Tool> = tools.iter().map(|t| t.definition.clone()).collect();

        // Create a map for quick tool lookup
        let tool_map: HashMap<&str, &ExecutableTool> =
            tools.iter().map(|t| (t.name(), t)).collect();

        let mut iterations = 0;
        let mut total_usage: Option<Usage> = None;

        // The agentic loop
        while iterations < max_iterations {
            iterations += 1;

            // Call the API
            let response = self.call_api(&model, &messages, Some(&api_tools)).await?;

            // Accumulate usage
            if let Some(usage) = &response.usage {
                if let Some(ref mut total) = total_usage {
                    total.prompt_tokens += usage.prompt_tokens;
                    total.completion_tokens += usage.completion_tokens;
                    total.total_tokens += usage.total_tokens;
                } else {
                    total_usage = Some(usage.clone());
                }
            }

            // Get the first choice
            let choice = match response.choices.first() {
                Some(c) => c,
                None => {
                    // No choices, return what we have
                    return Ok(SendResponse {
                        usage: total_usage,
                        ..response
                    });
                }
            };

            // Check for tool calls
            let tool_calls = match &choice.message.tool_calls {
                Some(calls) if !calls.is_empty() => calls,
                _ => {
                    // No tool calls, we're done - return final response with accumulated usage
                    return Ok(SendResponse {
                        usage: total_usage,
                        ..response
                    });
                }
            };

            // Add assistant's response (with tool_calls) to messages
            messages.push(choice.message.clone());

            // Execute each tool call and add results
            for tool_call in tool_calls {
                let tool_name = &tool_call.function.name;

                let result = if let Some(tool) = tool_map.get(tool_name.as_str()) {
                    // Parse arguments and execute
                    match serde_json::from_str::<serde_json::Value>(&tool_call.function.arguments) {
                        Ok(args) => {
                            let result = tool.execute(args).await;
                            if result.is_string() {
                                result.as_str().unwrap_or("").to_string()
                            } else {
                                serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
                            }
                        }
                        Err(e) => {
                            json!({"error": format!("Invalid arguments: {}", e)}).to_string()
                        }
                    }
                } else {
                    json!({"error": format!("Unknown tool: {}", tool_name)}).to_string()
                };

                // Add tool result to messages
                messages.push(Message::tool(&tool_call.id, result));
            }

            // Loop continues - model will process tool results
        }

        // Max iterations reached
        Err(Error::MaxIterationsExceeded(max_iterations))
    }

    /// Internal helper to call the API
    async fn call_api(
        &self,
        model: &str,
        messages: &[Message],
        tools: Option<&[Tool]>,
    ) -> Result<SendResponse> {
        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": false,
        });

        if let Some(tools) = tools {
            body["tools"] = json!(tools);
        }

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::Api { status, message });
        }

        let send_response: SendResponse = response.json().await?;
        Ok(send_response)
    }
}

/// Builder for send_with_tools requests
///
/// Allows optional configuration like max_iterations before executing the request.
pub struct SendWithToolsBuilder {
    client: Edgee,
    model: String,
    input: String,
    tools: Vec<ExecutableTool>,
    max_iterations: u32,
}

impl SendWithToolsBuilder {
    fn new(client: Edgee, model: String, input: String, tools: Vec<ExecutableTool>) -> Self {
        Self {
            client,
            model,
            input,
            tools,
            max_iterations: 10, // Default matching TypeScript SDK
        }
    }

    /// Set the maximum number of tool execution iterations
    ///
    /// Default is 10. If the model keeps requesting tool calls beyond this limit,
    /// an error is returned.
    pub fn max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    /// Execute the request and return the final response
    pub async fn execute(self) -> Result<SendResponse> {
        self.client
            .execute_with_tools(self.model, self.input, self.tools, self.max_iterations)
            .await
    }
}

// Implement IntoFuture for ergonomic .await directly on the builder
impl std::future::IntoFuture for SendWithToolsBuilder {
    type Output = Result<SendResponse>;
    type IntoFuture = Pin<Box<dyn std::future::Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.execute())
    }
}

/// Builder for stream_with_tools requests
///
/// Allows optional configuration like max_iterations before executing the request.
pub struct StreamWithToolsBuilder {
    client: Edgee,
    model: String,
    input: String,
    tools: Vec<ExecutableTool>,
    max_iterations: u32,
}

impl StreamWithToolsBuilder {
    fn new(client: Edgee, model: String, input: String, tools: Vec<ExecutableTool>) -> Self {
        Self {
            client,
            model,
            input,
            tools,
            max_iterations: 10,
        }
    }

    /// Set the maximum number of tool execution iterations
    ///
    /// Default is 10. If the model keeps requesting tool calls beyond this limit,
    /// an error is returned.
    pub fn max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    /// Execute the streaming request and return a stream of events
    pub async fn execute(
        self,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let client = self.client;
        let model = self.model;
        let input = self.input;
        let tools = self.tools;
        let max_iterations = self.max_iterations;

        // Build initial messages
        let mut messages: Vec<Message> = vec![Message::user(input)];

        // Convert ExecutableTool to API format
        let api_tools: Vec<Tool> = tools.iter().map(|t| t.definition.clone()).collect();

        // Create a map for quick tool lookup
        let tool_map: HashMap<String, ExecutableTool> = tools
            .into_iter()
            .map(|t| (t.name().to_string(), t))
            .collect();

        let stream = try_stream! {
            for iteration in 1..=max_iterations {
                // Accumulate the full response from stream
                let mut role: Option<Role> = None;
                let mut content = String::new();
                let mut tool_calls_accumulator: HashMap<u32, ToolCall> = HashMap::new();

                // Stream the response
                let mut body = json!({
                    "model": &model,
                    "messages": &messages,
                    "stream": true,
                });
                body["tools"] = json!(&api_tools);

                let response = client
                    .client
                    .post(format!("{}/v1/chat/completions", client.config.base_url))
                    .header("Authorization", format!("Bearer {}", client.config.api_key))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await?;

                let status = response.status();
                let status_code = status.as_u16();

                // Must get byte_stream before any potential consumption of response
                let byte_stream = if status.is_success() {
                    Some(response.bytes_stream())
                } else {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    Err(Error::Api { status: status_code, message })?
                };

                let mut chunk_stream = Edgee::parse_sse_stream(byte_stream.unwrap());

                while let Some(result) = chunk_stream.next().await {
                    let chunk = result?;

                    // Yield the chunk as an event
                    yield StreamEvent::Chunk(chunk.clone());

                    // Accumulate role
                    if let Some(r) = chunk.role() {
                        role = Some(r.clone());
                    }

                    // Accumulate content
                    if let Some(text) = chunk.text() {
                        content.push_str(text);
                    }

                    // Accumulate tool calls from deltas
                    if let Some(deltas) = chunk.tool_call_deltas() {
                        for delta in deltas {
                            // Use index from the delta, defaulting to parsing from id
                            let idx = delta.id.parse::<u32>().unwrap_or(0);
                            if let Some(existing) = tool_calls_accumulator.get_mut(&idx) {
                                // Append arguments to existing tool call
                                existing.function.arguments.push_str(&delta.function.arguments);
                            } else {
                                // Start new tool call
                                tool_calls_accumulator.insert(idx, delta.clone());
                            }
                        }
                    }
                }

                // Convert accumulated tool calls to vec
                let tool_calls: Vec<ToolCall> = tool_calls_accumulator.into_values().collect();

                // No tool calls? We're done
                if tool_calls.is_empty() {
                    return;
                }

                // Add assistant's message (with tool_calls) to messages
                let assistant_msg = Message {
                    role: role.unwrap_or(Role::Assistant),
                    content: if content.is_empty() { None } else { Some(content) },
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                };
                messages.push(assistant_msg);

                // Execute each tool call and add results
                for tool_call in tool_calls {
                    let tool_name = &tool_call.function.name;

                    // Yield tool_start event
                    yield StreamEvent::ToolStart { tool_call: tool_call.clone() };

                    let result = if let Some(tool) = tool_map.get(tool_name) {
                        match serde_json::from_str::<serde_json::Value>(&tool_call.function.arguments) {
                            Ok(args) => tool.execute(args).await,
                            Err(e) => json!({"error": format!("Invalid arguments: {}", e)}),
                        }
                    } else {
                        json!({"error": format!("Unknown tool: {}", tool_name)})
                    };

                    // Yield tool_result event
                    yield StreamEvent::ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        tool_name: tool_name.clone(),
                        result: result.clone(),
                    };

                    // Add tool result to messages
                    let result_str = if result.is_string() {
                        result.as_str().unwrap_or("").to_string()
                    } else {
                        serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
                    };
                    messages.push(Message::tool(&tool_call.id, result_str));
                }

                // Yield iteration complete event
                yield StreamEvent::IterationComplete { iteration };

                // Loop continues - model will process tool results
            }

            // Max iterations reached
            Err(Error::MaxIterationsExceeded(max_iterations))?;
        };

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_conversions() {
        let _input: Input = "hello".into();
        let _input: Input = "hello".to_string().into();
        let _input: Input = InputObject::new(vec![Message::user("hello")]).into();
        let _input: Input = vec![Message::user("hello")].into();
    }

    #[test]
    fn test_config_from_env() {
        unsafe {
            std::env::set_var("EDGEE_API_KEY", "test-key");
            std::env::set_var("EDGEE_BASE_URL", "https://test.example.com");
        }

        let config = EdgeeConfig::from_env().unwrap();
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.base_url, "https://test.example.com");

        unsafe {
            std::env::remove_var("EDGEE_API_KEY");
            std::env::remove_var("EDGEE_BASE_URL");
        }
    }

    #[test]
    fn test_config_builder() {
        let config = EdgeeConfig::new("my-key").with_base_url("https://custom.example.com");

        assert_eq!(config.api_key, "my-key");
        assert_eq!(config.base_url, "https://custom.example.com");
    }

    #[test]
    fn test_message_constructors() {
        let msg = Message::user("hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content.as_deref(), Some("hello"));

        let msg = Message::system("You are helpful");
        assert_eq!(msg.role, Role::System);

        let msg = Message::developer("You are helpful");
        assert_eq!(msg.role, Role::Developer);

        let msg = Message::assistant("Hi there");
        assert_eq!(msg.role, Role::Assistant);

        let msg = Message::tool("call-123", "result");
        assert_eq!(msg.role, Role::Tool);
        assert_eq!(msg.tool_call_id.as_deref(), Some("call-123"));
    }
}
