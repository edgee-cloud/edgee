use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

impl InputObject {
    /// Create a new input with messages
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            tools: None,
            tool_choice: None,
            tags: None,
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

    /// Set tags for the request
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
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
}
