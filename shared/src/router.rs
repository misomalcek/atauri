use crate::models::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The Weaver - routes tasks between models based on complexity spectrum
/// Maps to Codex Symbiosis: temperature management + orchestration
///
/// Three zones instead of a binary threshold:
/// - LOCAL zone (below local_ceiling): always local - fast, private, free
/// - WEAVER zone (between): nuanced decision based on task type, context, depth
/// - CLOUD zone (above cloud_floor): always cloud - powerful reasoning
pub struct Router {
    models: Vec<Arc<dyn AiModel>>,
    pub config: RoutingConfig,
}

/// Routing configuration with spectrum/range instead of hard threshold
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    /// Below this score → always local (default 0.25)
    pub local_ceiling: f32,
    /// Above this score → always cloud (default 0.65)
    pub cloud_floor: f32,
    /// In the Weaver zone, prefer local for code-focused tasks
    pub prefer_local_for_code: bool,
    /// In the Weaver zone, prefer cloud for reasoning/analysis
    pub prefer_cloud_for_reasoning: bool,
    /// Escalate to cloud as conversation gets deeper
    pub conversation_depth_factor: bool,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            local_ceiling: 0.25,
            cloud_floor: 0.65,
            prefer_local_for_code: true,
            prefer_cloud_for_reasoning: true,
            conversation_depth_factor: true,
        }
    }
}

impl RoutingConfig {
    /// Create from a single threshold (backward compatibility)
    pub fn from_threshold(threshold: f32) -> Self {
        Self {
            local_ceiling: (threshold - 0.15).max(0.1),
            cloud_floor: (threshold + 0.15).min(0.9),
            ..Default::default()
        }
    }
}

/// Detailed analysis of a request's characteristics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityProfile {
    pub score: f32,
    pub is_code_task: bool,
    pub is_reasoning_task: bool,
    pub requires_creativity: bool,
    pub conversation_depth: usize,
    pub context_size: usize,
    pub zone: RoutingZone,
}

/// Which zone the request falls into
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoutingZone {
    Local,
    Weaver,
    Cloud,
}

impl std::fmt::Display for RoutingZone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoutingZone::Local => write!(f, "LOCAL"),
            RoutingZone::Weaver => write!(f, "WEAVER"),
            RoutingZone::Cloud => write!(f, "CLOUD"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDecision {
    pub model_name: String,
    pub tier: ModelTier,
    pub complexity: f32,
    pub reason: String,
    #[serde(default)]
    pub temperature_mode: TemperatureMode,
}

/// Temperature modes from Codex Symbiosis
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum TemperatureMode {
    Stabilize,
    #[default]
    Weave,
    Ignite,
}

impl TemperatureMode {
    pub fn temperature(&self) -> f32 {
        match self {
            TemperatureMode::Stabilize => 0.2,
            TemperatureMode::Weave => 0.5,
            TemperatureMode::Ignite => 0.9,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            TemperatureMode::Stabilize => "STABILIZE",
            TemperatureMode::Weave => "WEAVE",
            TemperatureMode::Ignite => "IGNITE",
        }
    }
}

impl Router {
    pub fn new(models: Vec<Arc<dyn AiModel>>, threshold: f32) -> Self {
        Self {
            models,
            config: RoutingConfig::from_threshold(threshold),
        }
    }

    pub fn new_with_config(models: Vec<Arc<dyn AiModel>>, config: RoutingConfig) -> Self {
        Self { models, config }
    }

    /// Analyze complexity with full profile - not just a number but task characteristics
    pub fn analyze_complexity(messages: &[Message]) -> ComplexityProfile {
        let last_msg = messages.last().map(|m| &m.content).cloned().unwrap_or_default();
        let lower = last_msg.to_lowercase();
        let full_context: String = messages.iter().map(|m| m.content.as_str()).collect::<Vec<_>>().join(" ");

        // Base complexity signals (same as before, expanded)
        let signals: Vec<bool> = vec![
            last_msg.len() > 500,
            full_context.len() > 2000,
            lower.contains("analyze") || lower.contains("compare")
                || lower.contains("design") || lower.contains("architect")
                || lower.contains("explain why") || lower.contains("refactor"),
            lower.contains("implement") || lower.contains("write a function")
                || lower.contains("create a class") || lower.contains("build"),
            last_msg.matches('?').count() > 2,
            lower.contains("best practice") || lower.contains("trade-off")
                || lower.contains("pros and cons"),
            messages.len() > 10,
            full_context.contains("```"),
        ];

        let score = signals.iter().filter(|&&s| s).count() as f32 / signals.len() as f32;

        // Task type detection
        let is_code_task = lower.contains("code") || lower.contains("function")
            || lower.contains("implement") || lower.contains("fix")
            || lower.contains("debug") || lower.contains("compile")
            || lower.contains("syntax") || lower.contains("variable")
            || full_context.contains("```");

        let is_reasoning_task = lower.contains("why") || lower.contains("explain")
            || lower.contains("analyze") || lower.contains("compare")
            || lower.contains("evaluate") || lower.contains("strategy")
            || lower.contains("design") || lower.contains("plan")
            || lower.contains("architecture");

        let requires_creativity = lower.contains("brainstorm") || lower.contains("idea")
            || lower.contains("creative") || lower.contains("imagine")
            || lower.contains("what if") || lower.contains("alternative")
            || lower.contains("innovate") || lower.contains("explore");

        ComplexityProfile {
            score,
            is_code_task,
            is_reasoning_task,
            requires_creativity,
            conversation_depth: messages.len(),
            context_size: full_context.len(),
            zone: RoutingZone::Weaver, // placeholder, set by router
        }
    }

    /// Backward-compatible score_complexity (returns just the f32)
    pub fn score_complexity(messages: &[Message]) -> f32 {
        Self::analyze_complexity(messages).score
    }

    /// Auto-detect temperature mode
    pub fn detect_temperature(messages: &[Message]) -> TemperatureMode {
        let last_msg = messages.last().map(|m| m.content.to_lowercase()).unwrap_or_default();

        if last_msg.contains("fix") || last_msg.contains("bug")
            || last_msg.contains("error") || last_msg.contains("compile")
            || last_msg.contains("exact") || last_msg.contains("specific")
        {
            return TemperatureMode::Stabilize;
        }

        if last_msg.contains("brainstorm") || last_msg.contains("idea")
            || last_msg.contains("creative") || last_msg.contains("explore")
            || last_msg.contains("what if") || last_msg.contains("imagine")
            || last_msg.contains("alternative") || last_msg.contains("innovate")
        {
            return TemperatureMode::Ignite;
        }

        TemperatureMode::Weave
    }

    /// The Weaver's decision in the fuzzy zone
    /// Weighs multiple factors to decide if local or cloud is better
    fn weave_decision(&self, profile: &ComplexityProfile) -> ModelTier {
        let mut cloud_weight: f32 = 0.0;

        // Base: normalized position within the Weaver zone
        let range = self.config.cloud_floor - self.config.local_ceiling;
        if range > 0.0 {
            let position = (profile.score - self.config.local_ceiling) / range;
            cloud_weight += position * 0.4;
        }

        // Factor: code tasks lean local (fast iteration, privacy)
        if profile.is_code_task && self.config.prefer_local_for_code {
            cloud_weight -= 0.15;
        }

        // Factor: reasoning/analysis tasks lean cloud (deeper thinking)
        if profile.is_reasoning_task && self.config.prefer_cloud_for_reasoning {
            cloud_weight += 0.15;
        }

        // Factor: deep conversations escalate (context accumulates)
        if self.config.conversation_depth_factor && profile.conversation_depth > 8 {
            cloud_weight += 0.1;
        }

        // Factor: large context benefits from cloud (bigger context window)
        if profile.context_size > 3000 {
            cloud_weight += 0.1;
        }

        // Factor: creativity benefits from cloud
        if profile.requires_creativity {
            cloud_weight += 0.1;
        }

        if cloud_weight >= 0.5 {
            ModelTier::Cloud
        } else {
            ModelTier::Local
        }
    }

    /// Route a request using the three-zone spectrum
    pub async fn route(&self, messages: &[Message], config: &InferenceConfig) -> Result<(RouteDecision, String), ModelError> {
        let mut profile = Self::analyze_complexity(messages);
        let temp_mode = Self::detect_temperature(messages);

        // Determine zone and target tier
        let (target_tier, zone) = if profile.score < self.config.local_ceiling {
            (ModelTier::Local, RoutingZone::Local)
        } else if profile.score > self.config.cloud_floor {
            (ModelTier::Cloud, RoutingZone::Cloud)
        } else {
            let tier = self.weave_decision(&profile);
            (tier, RoutingZone::Weaver)
        };
        profile.zone = zone;

        // Apply temperature mode
        let mut adjusted_config = config.clone();
        if adjusted_config.temperature == 0.7 {
            adjusted_config.temperature = temp_mode.temperature();
        }

        let primary = self.models.iter().find(|m| m.tier() == target_tier);
        let fallback = self.models.iter().find(|m| m.tier() != target_tier);

        let zone_label = format!("{}", profile.zone);
        let reason_detail = if matches!(profile.zone, RoutingZone::Weaver) {
            let factors = vec![
                if profile.is_code_task { Some("code") } else { None },
                if profile.is_reasoning_task { Some("reasoning") } else { None },
                if profile.requires_creativity { Some("creative") } else { None },
                if profile.conversation_depth > 8 { Some("deep-conv") } else { None },
            ].into_iter().flatten().collect::<Vec<_>>().join("+");
            format!("{} zone [{factors}]", zone_label)
        } else {
            format!("{} zone", zone_label)
        };

        if let Some(model) = primary {
            if model.is_available().await {
                let msgs = Self::inject_identity(messages, model.name(), model.tier());
                match model.chat(&msgs, &adjusted_config).await {
                    Ok(response) => {
                        let decision = RouteDecision {
                            model_name: model.name().to_string(),
                            tier: model.tier(),
                            complexity: profile.score,
                            reason: format!("{} | {} {:.2} → {:?}", temp_mode.label(), reason_detail, profile.score, model.tier()),
                            temperature_mode: temp_mode,
                        };
                        return Ok((decision, response));
                    }
                    Err(e) => {
                        eprintln!("Primary model failed: {e}, trying fallback...");
                    }
                }
            }
        }

        if let Some(model) = fallback {
            if model.is_available().await {
                let msgs = Self::inject_identity(messages, model.name(), model.tier());
                let response = model.chat(&msgs, &adjusted_config).await?;
                let decision = RouteDecision {
                    model_name: model.name().to_string(),
                    tier: model.tier(),
                    complexity: profile.score,
                    reason: format!("{} | Fallback to {:?}", temp_mode.label(), model.tier()),
                    temperature_mode: temp_mode,
                };
                return Ok((decision, response));
            }
        }

        Err(ModelError::ApiError("No models available".into()))
    }

    /// Force a specific model by name (user override)
    pub async fn route_to(&self, model_name: &str, messages: &[Message], config: &InferenceConfig) -> Result<String, ModelError> {
        let model = self
            .models
            .iter()
            .find(|m| m.name() == model_name)
            .ok_or(ModelError::ModelNotLoaded(format!("Model '{model_name}' not found")))?;

        let msgs = Self::inject_identity(messages, model.name(), model.tier());
        model.chat(&msgs, config).await
    }

    /// Snow Globe: query ALL available models in parallel
    pub async fn deliberate(&self, messages: &[Message], config: &InferenceConfig) -> Vec<(String, Result<String, ModelError>)> {
        let mut handles = Vec::new();

        for model in &self.models {
            let model = Arc::clone(model);
            let msgs = messages.to_vec();
            let cfg = config.clone();

            let handle = tokio::spawn(async move {
                let name = model.name().to_string();
                if model.is_available().await {
                    let result = model.chat(&msgs, &cfg).await;
                    (name, result)
                } else {
                    (name, Err(ModelError::ModelNotLoaded("Not available".into())))
                }
            });

            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            if let Ok(result) = handle.await {
                results.push(result);
            }
        }

        results
    }

    pub fn available_models(&self) -> Vec<(&str, ModelTier)> {
        self.models.iter().map(|m| (m.name(), m.tier())).collect()
    }

    /// Inject model identity into the system message
    fn inject_identity(messages: &[Message], model_name: &str, tier: ModelTier) -> Vec<Message> {
        let tier_label = match tier {
            ModelTier::Local => "local",
            ModelTier::Cloud => "cloud",
        };
        let identity = format!(
            "\n\n[Model Identity: You are {model_name}, a {tier_label} AI model running inside Tauri Code. \
             You are part of a multi-model system called The Weaver that routes between local and cloud models. \
             When asked who you are, identify yourself as {model_name}.]"
        );

        let mut msgs = messages.to_vec();
        if let Some(first) = msgs.first_mut() {
            if first.role == "system" {
                first.content.push_str(&identity);
                return msgs;
            }
        }
        msgs.insert(0, Message {
            role: "system".into(),
            content: identity,
        });
        msgs
    }
}
