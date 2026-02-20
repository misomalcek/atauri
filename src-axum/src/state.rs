use tc_core::context::ProjectContext;
use tc_core::ghost::Ghost;
use tc_core::models::Message;
use tc_core::prompt::PromptEngine;
use tc_core::qdrant::KnowledgeBase;
use tc_core::router::Router;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Shared application state for the web backend
/// Mirror of the Tauri AppState, but without Tauri dependencies
#[derive(Clone)]
pub struct AppState {
    pub router: Arc<Mutex<Option<Router>>>,
    pub ghost: Arc<Mutex<Option<Ghost>>>,
    pub project: Arc<Mutex<Option<ProjectContext>>>,
    pub prompt_engine: Arc<Mutex<Option<PromptEngine>>>,
    pub knowledge_base: Arc<Mutex<Option<KnowledgeBase>>>,
    pub config: Arc<Mutex<AppConfig>>,
    pub conversation: Arc<Mutex<Vec<Message>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub lm_studio_port: u16,
    pub claude_api_key: Option<String>,
    pub qdrant_url: Option<String>,
    #[serde(default = "default_threshold")]
    pub complexity_threshold: f32,
    pub default_model: Option<String>,
    #[serde(default = "default_local_ceiling")]
    pub local_ceiling: f32,
    #[serde(default = "default_cloud_floor")]
    pub cloud_floor: f32,
    #[serde(default = "default_true")]
    pub prefer_local_for_code: bool,
    #[serde(default = "default_true")]
    pub prefer_cloud_for_reasoning: bool,
    #[serde(default = "default_true")]
    pub conversation_depth_factor: bool,
}

fn default_threshold() -> f32 { 0.4 }
fn default_local_ceiling() -> f32 { 0.25 }
fn default_cloud_floor() -> f32 { 0.65 }
fn default_true() -> bool { true }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            lm_studio_port: 1234,
            claude_api_key: None,
            qdrant_url: Some("http://localhost:6333".into()),
            complexity_threshold: 0.4,
            default_model: None,
            local_ceiling: 0.25,
            cloud_floor: 0.65,
            prefer_local_for_code: true,
            prefer_cloud_for_reasoning: true,
            conversation_depth_factor: true,
        }
    }
}

impl AppConfig {
    pub fn to_routing_config(&self) -> tc_core::router::RoutingConfig {
        tc_core::router::RoutingConfig {
            local_ceiling: self.local_ceiling,
            cloud_floor: self.cloud_floor,
            prefer_local_for_code: self.prefer_local_for_code,
            prefer_cloud_for_reasoning: self.prefer_cloud_for_reasoning,
            conversation_depth_factor: self.conversation_depth_factor,
        }
    }
}

impl AppConfig {
    fn config_path() -> PathBuf {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        home.join(".tauri-code").join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let mut config = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|data| serde_json::from_str::<AppConfig>(&data).ok())
                .unwrap_or_default()
        } else {
            AppConfig::default()
        };
        // Env var overrides (for Docker/VPS deployment)
        if let Ok(url) = std::env::var("QDRANT_URL") {
            config.qdrant_url = Some(url);
        }
        if let Ok(key) = std::env::var("CLAUDE_API_KEY") {
            config.claude_api_key = Some(key);
        }
        config
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        if let Ok(data) = serde_json::to_string_pretty(self) {
            std::fs::write(path, data).ok();
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        let config = AppConfig::load();
        Self {
            router: Arc::new(Mutex::new(None)),
            ghost: Arc::new(Mutex::new(None)),
            project: Arc::new(Mutex::new(None)),
            prompt_engine: Arc::new(Mutex::new(None)),
            knowledge_base: Arc::new(Mutex::new(None)),
            config: Arc::new(Mutex::new(config)),
            conversation: Arc::new(Mutex::new(Vec::new())),
        }
    }
}
