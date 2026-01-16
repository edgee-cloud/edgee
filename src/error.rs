/// Errors that can occur when using the Edgee SDK
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization failed
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// API key is missing
    #[error("API key is required. Set EDGEE_API_KEY environment variable or provide in config")]
    MissingApiKey,

    /// API returned an error
    #[error("API error (status {status}): {message}")]
    Api { status: u16, message: String },

    /// Streaming error
    #[error("Streaming error: {0}")]
    Stream(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Maximum tool iterations exceeded
    #[error("Maximum tool iterations ({0}) exceeded")]
    MaxIterationsExceeded(u32),

    /// Tool execution error
    #[error("Tool execution error for '{tool_name}': {message}")]
    ToolExecution { tool_name: String, message: String },
}

/// Result type alias for Edgee operations
pub type Result<T> = std::result::Result<T, Error>;
