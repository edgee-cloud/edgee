use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Configuration for the Edgee client
#[derive(Debug, Clone)]
pub struct EdgeeConfig {
    /// API key for authentication
    pub api_key: String,
    /// Base URL for the API (default: <https://api.edgee.ai>)
    pub base_url: String,
}

impl EdgeeConfig {
    /// Create a new configuration with the given API key
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.edgee.ai".to_string(),
        }
    }

    /// Set a custom base URL
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Create configuration from environment variables
    /// Reads EDGEE_API_KEY and optionally EDGEE_BASE_URL
    pub fn from_env() -> crate::Result<Self> {
        let api_key = std::env::var("EDGEE_API_KEY").map_err(|_| crate::Error::MissingApiKey)?;

        let base_url =
            std::env::var("EDGEE_BASE_URL").unwrap_or_else(|_| "https://api.edgee.ai".to_string());

        Ok(Self { api_key, base_url })
    }
}

/// Message role in a conversation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    Developer,
    User,
    Assistant,
    Tool,
}

/// Function call made by the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// Tool call information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// Message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a developer message
    pub fn developer(content: impl Into<String>) -> Self {
        Self {
            role: Role::Developer,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a tool response message
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// JSON Schema for function parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Function definition for tool calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: JsonSchema,
}

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

impl Tool {
    /// Create a new function tool
    pub fn function(function: FunctionDefinition) -> Self {
        Self {
            tool_type: "function".to_string(),
            function,
        }
    }
}

/// Type alias for async tool handler function
pub type ToolHandlerFn = Arc<
    dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = serde_json::Value> + Send>>
        + Send
        + Sync,
>;

/// A tool with an executable handler for automatic tool calling
///
/// This is used in "simple mode" where the SDK automatically executes
/// tool calls and feeds results back to the model.
///
/// # Example
/// ```no_run
/// use edgee::{ExecutableTool, JsonSchema};
/// use serde_json::json;
/// use std::collections::HashMap;
///
/// let tool = ExecutableTool::new(
///     "get_weather",
///     "Get the weather for a location",
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
///         json!({"temperature": 72, "location": location})
///     },
/// );
/// ```
#[derive(Clone)]
pub struct ExecutableTool {
    /// The tool definition (sent to the API)
    pub definition: Tool,
    /// The handler function to execute when this tool is called
    handler: ToolHandlerFn,
}

impl std::fmt::Debug for ExecutableTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutableTool")
            .field("definition", &self.definition)
            .field("handler", &"<async fn>")
            .finish()
    }
}

impl ExecutableTool {
    /// Create a new executable tool with an async handler
    ///
    /// # Arguments
    /// * `name` - The name of the tool/function
    /// * `description` - A description of what the tool does
    /// * `parameters` - JSON schema describing the parameters
    /// * `handler` - An async function that takes parsed arguments and returns a result
    pub fn new<F, Fut>(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: JsonSchema,
        handler: F,
    ) -> Self
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = serde_json::Value> + Send + 'static,
    {
        let handler = Arc::new(handler);
        Self {
            definition: Tool::function(FunctionDefinition {
                name: name.into(),
                description: Some(description.into()),
                parameters,
            }),
            handler: Arc::new(move |args| {
                let handler = handler.clone();
                Box::pin(async move { handler(args).await })
            }),
        }
    }

    /// Get the name of this tool
    pub fn name(&self) -> &str {
        &self.definition.function.name
    }

    /// Execute the tool handler with the given arguments
    pub async fn execute(&self, args: serde_json::Value) -> serde_json::Value {
        (self.handler)(args).await
    }
}

/// Tool choice configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    /// Auto-select tools
    Auto,
    /// Never use tools
    None,
    /// Use a specific tool
    Specific {
        r#type: String,
        function: HashMap<String, String>,
    },
}

/// Input for the chat completion request
#[derive(Debug, Clone, Serialize)]
pub struct InputObject {
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
}

impl InputObject {
    /// Create a new input with messages
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            tools: None,
            tool_choice: None,
        }
    }

    /// Add tools to the input
    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set tool choice
    pub fn with_tool_choice(mut self, tool_choice: serde_json::Value) -> Self {
        self.tool_choice = Some(tool_choice);
        self
    }
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Choice in a non-streaming response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: Option<String>,
}

/// Response from a non-streaming request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

impl SendResponse {
    /// Get the text content from the first choice
    pub fn text(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|c| c.message.content.as_deref())
    }

    /// Get the message from the first choice
    pub fn message(&self) -> Option<&Message> {
        self.choices.first().map(|c| &c.message)
    }

    /// Get the finish reason from the first choice
    pub fn finish_reason(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|c| c.finish_reason.as_deref())
    }

    /// Get tool calls from the first choice
    pub fn tool_calls(&self) -> Option<&Vec<ToolCall>> {
        self.choices
            .first()
            .and_then(|c| c.message.tool_calls.as_ref())
    }
}

/// Delta in a streaming response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// Choice in a streaming response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChoice {
    pub index: u32,
    pub delta: StreamDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

/// Chunk in a streaming response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<StreamChoice>,
}

impl StreamChunk {
    /// Get the text content from the first choice delta
    pub fn text(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|c| c.delta.content.as_deref())
    }

    /// Get the role from the first choice delta
    pub fn role(&self) -> Option<&Role> {
        self.choices.first().and_then(|c| c.delta.role.as_ref())
    }

    /// Get the finish reason from the first choice
    pub fn finish_reason(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|c| c.finish_reason.as_deref())
    }

    /// Get tool call deltas from the first choice
    pub fn tool_call_deltas(&self) -> Option<&Vec<ToolCall>> {
        self.choices
            .first()
            .and_then(|c| c.delta.tool_calls.as_ref())
    }
}

/// Stream events for tool-enabled streaming
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A chunk of streamed content
    Chunk(StreamChunk),
    /// Tool execution is starting
    ToolStart { tool_call: ToolCall },
    /// Tool execution completed
    ToolResult {
        tool_call_id: String,
        tool_name: String,
        result: serde_json::Value,
    },
    /// One iteration of the tool loop completed
    IterationComplete { iteration: u32 },
}

impl StreamEvent {
    /// Get text if this is a chunk event
    pub fn text(&self) -> Option<&str> {
        match self {
            StreamEvent::Chunk(chunk) => chunk.text(),
            _ => None,
        }
    }

    /// Check if this is a chunk event
    pub fn is_chunk(&self) -> bool {
        matches!(self, StreamEvent::Chunk(_))
    }

    /// Check if this is a tool start event
    pub fn is_tool_start(&self) -> bool {
        matches!(self, StreamEvent::ToolStart { .. })
    }

    /// Check if this is a tool result event
    pub fn is_tool_result(&self) -> bool {
        matches!(self, StreamEvent::ToolResult { .. })
    }
}

/// Builder for creating executable tools with a fluent API
///
/// # Example
/// ```no_run
/// use edgee::ToolBuilder;
/// use serde_json::json;
///
/// let tool = ToolBuilder::new("get_weather")
///     .description("Get the weather for a location")
///     .param("location", json!({"type": "string", "description": "City name"}))
///     .param("unit", json!({"type": "string", "enum": ["celsius", "fahrenheit"]}))
///     .required(vec!["location"])
///     .handler(|args| async move {
///         let location = args["location"].as_str().unwrap_or("Unknown");
///         json!({"temperature": 72, "location": location})
///     })
///     .build();
/// ```
pub struct ToolBuilder {
    name: String,
    description: Option<String>,
    properties: HashMap<String, serde_json::Value>,
    required: Vec<String>,
}

impl ToolBuilder {
    /// Create a new tool builder with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            properties: HashMap::new(),
            required: Vec::new(),
        }
    }

    /// Set the tool description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add a parameter to the tool
    pub fn param(mut self, name: impl Into<String>, schema: serde_json::Value) -> Self {
        self.properties.insert(name.into(), schema);
        self
    }

    /// Set required parameters
    pub fn required(mut self, required: Vec<&str>) -> Self {
        self.required = required.into_iter().map(String::from).collect();
        self
    }

    /// Build the executable tool with the given handler
    pub fn handler<F, Fut>(self, handler: F) -> ExecutableTool
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = serde_json::Value> + Send + 'static,
    {
        ExecutableTool::new(
            self.name,
            self.description.unwrap_or_default(),
            JsonSchema {
                schema_type: "object".to_string(),
                properties: if self.properties.is_empty() {
                    None
                } else {
                    Some(self.properties)
                },
                required: if self.required.is_empty() {
                    None
                } else {
                    Some(self.required)
                },
                description: None,
            },
            handler,
        )
    }
}

/// Macro to create an ExecutableTool with less boilerplate
///
/// # Examples
///
/// Basic usage:
/// ```no_run
/// use edgee::tool;
/// use serde_json::json;
///
/// let weather = tool!(
///     "get_weather",
///     "Get the weather for a location",
///     {
///         "location" => {"type": "string", "description": "City name"},
///         "unit" => {"type": "string", "enum": ["celsius", "fahrenheit"]}
///     },
///     required: ["location"],
///     |args| async move {
///         let location = args["location"].as_str().unwrap_or("Unknown");
///         json!({"temperature": 72, "location": location})
///     }
/// );
/// ```
///
/// Without parameters:
/// ```no_run
/// use edgee::tool;
/// use serde_json::json;
///
/// let time = tool!(
///     "get_time",
///     "Get the current time",
///     |_args| async move { json!({"time": "12:00"}) }
/// );
/// ```
#[macro_export]
macro_rules! tool {
    // With parameters and required fields
    (
        $name:expr,
        $description:expr,
        { $($param_name:expr => $param_schema:tt),* $(,)? },
        required: [$($required:expr),* $(,)?],
        $handler:expr
    ) => {{
        $crate::ToolBuilder::new($name)
            .description($description)
            $(.param($param_name, serde_json::json!($param_schema)))*
            .required(vec![$($required),*])
            .handler($handler)
    }};

    // With parameters, no required fields
    (
        $name:expr,
        $description:expr,
        { $($param_name:expr => $param_schema:tt),* $(,)? },
        $handler:expr
    ) => {{
        $crate::ToolBuilder::new($name)
            .description($description)
            $(.param($param_name, serde_json::json!($param_schema)))*
            .handler($handler)
    }};

    // No parameters
    (
        $name:expr,
        $description:expr,
        $handler:expr
    ) => {{
        $crate::ToolBuilder::new($name)
            .description($description)
            .handler($handler)
    }};
}
