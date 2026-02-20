use super::*;
use reqwest::Client;

/// LM Studio client - OpenAI-compatible API on localhost
/// Supports any model loaded in LM Studio (Gemma, DevStral, Qwen, GPT-OSS...)
pub struct LmStudioModel {
    pub model_id: String,
    pub display_name: String,
    pub base_url: String,
    client: Client,
}

impl LmStudioModel {
    pub fn new(model_id: &str, display_name: &str, port: u16) -> Self {
        Self {
            model_id: model_id.to_string(),
            display_name: display_name.to_string(),
            base_url: format!("http://localhost:{port}"),
            client: Client::new(),
        }
    }

    /// Discover all models currently available in LM Studio
    pub async fn discover(port: u16) -> Vec<ModelInfo> {
        let client = Client::new();
        let url = format!("http://localhost:{port}/v1/models");

        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        let body: serde_json::Value = match resp.json().await {
            Ok(b) => b,
            Err(_) => return vec![],
        };

        let Some(models) = body["data"].as_array() else {
            return vec![];
        };

        models
            .iter()
            .filter_map(|m| {
                let id = m["id"].as_str()?;
                if id.contains("embed") {
                    return None;
                }
                Some(ModelInfo {
                    id: id.to_string(),
                    name: id.split('/').last().unwrap_or(id).to_string(),
                    tier: ModelTier::Local,
                    available: true,
                })
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl AiModel for LmStudioModel {
    fn name(&self) -> &str {
        &self.display_name
    }

    fn tier(&self) -> ModelTier {
        ModelTier::Local
    }

    async fn chat(&self, messages: &[Message], config: &InferenceConfig) -> Result<String, ModelError> {
        let payload = serde_json::json!({
            "model": self.model_id,
            "messages": messages,
            "temperature": config.temperature,
            "max_tokens": config.max_tokens,
        });

        let resp = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
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

            if error_msg.contains("insufficient system resources") {
                return Err(ModelError::OutOfMemory(error_msg.to_string()));
            }
            return Err(ModelError::ApiError(error_msg.to_string()));
        }

        body["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or(ModelError::ApiError("No content in response".into()))
    }

    async fn is_available(&self) -> bool {
        self.client
            .get(format!("{}/v1/models", self.base_url))
            .send()
            .await
            .is_ok()
    }
}
