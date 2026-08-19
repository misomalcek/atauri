use super::*;
use reqwest::Client;

/// Claude API client - cloud-based, powerful reasoning
pub struct ClaudeModel {
    pub model_id: String,
    pub display_name: String,
    api_key: String,
    client: Client,
}

impl ClaudeModel {
    pub fn new(model_id: &str, display_name: &str, api_key: &str) -> Self {
        Self {
            model_id: model_id.to_string(),
            display_name: display_name.to_string(),
            api_key: api_key.to_string(),
            client: Client::new(),
        }
    }

    /// Default Claude Sonnet - good balance of speed and capability
    pub fn sonnet(api_key: &str) -> Self {
        Self::new("claude-sonnet-4-6", "Claude Sonnet 4.6", api_key)
    }

    /// Claude Opus - maximum reasoning
    pub fn opus(api_key: &str) -> Self {
        Self::new("claude-opus-4-6", "Claude Opus 4.6", api_key)
    }

    /// Claude Haiku - fast, lightweight tasks
    pub fn haiku(api_key: &str) -> Self {
        Self::new("claude-haiku-4-5-20251001", "Claude Haiku 4.5", api_key)
    }
}

#[async_trait::async_trait]
impl AiModel for ClaudeModel {
    fn name(&self) -> &str {
        &self.display_name
    }

    fn tier(&self) -> ModelTier {
        ModelTier::Cloud
    }

    async fn chat(&self, messages: &[Message], config: &InferenceConfig) -> Result<String, ModelError> {
        // Convert to Anthropic format (separate system from messages)
        let system_msg = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone());

        let chat_messages: Vec<serde_json::Value> = messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                })
            })
            .collect();

        let mut payload = serde_json::json!({
            "model": self.model_id,
            "messages": chat_messages,
            "max_tokens": config.max_tokens,
            "temperature": config.temperature,
        });

        if let Some(sys) = system_msg {
            payload["system"] = serde_json::Value::String(sys);
        }

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ModelError::NetworkError(e.to_string()))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ModelError::ApiError(e.to_string()))?;

        if !status.is_success() {
            let error_msg = body["error"]["message"]
                .as_str()
                .unwrap_or("Unknown error");
            return Err(ModelError::ApiError(error_msg.to_string()));
        }

        body["content"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or(ModelError::ApiError("No content in response".into()))
    }

    async fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }
}
