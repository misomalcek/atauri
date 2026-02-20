pub mod cloud;
pub mod local;

use serde::{Deserialize, Serialize};

/// Every model implements this trait - Protocol pattern from our earlier design
#[async_trait::async_trait]
pub trait AiModel: Send + Sync {
    fn name(&self) -> &str;
    fn tier(&self) -> ModelTier;
    async fn chat(&self, messages: &[Message], config: &InferenceConfig) -> Result<String, ModelError>;
    async fn is_available(&self) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ModelTier {
    Local,  // LM Studio, Ollama - fast, private, limited
    Cloud,  // Claude API - powerful, connected, costs tokens
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,     // "system", "user", "assistant"
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    pub temperature: f32,
    pub max_tokens: u32,
    pub stream: bool,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 2000,
            stream: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub tier: ModelTier,
    pub available: bool,
}

#[derive(Debug)]
pub enum ModelError {
    NetworkError(String),
    ApiError(String),
    ModelNotLoaded(String),
    OutOfMemory(String),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NetworkError(e) => write!(f, "Network error: {e}"),
            Self::ApiError(e) => write!(f, "API error: {e}"),
            Self::ModelNotLoaded(e) => write!(f, "Model not loaded: {e}"),
            Self::OutOfMemory(e) => write!(f, "Out of memory: {e}"),
        }
    }
}
