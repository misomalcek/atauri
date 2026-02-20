use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Core Prompt Engine - Dual prompt system
/// From Codex Symbiosis: system_prompt (user-configurable) + core_prompt (AI-evolving, append-only)
///
/// The ratio controls how much of the token budget goes to each.
/// User prefers 50:50 (not golden ratio default).
///
/// core_prompt is append-only - it grows as the AI learns about the project.
/// This is where "context first" meets "context evolution."
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptEngine {
    /// User-editable system prompt
    pub system_prompt: String,
    /// AI-evolving core prompt (append-only entries)
    pub core_entries: Vec<CoreEntry>,
    /// Ratio: 0.0 = all system, 1.0 = all core. Default 0.5 (50:50)
    pub ratio: f32,
    /// Where this engine persists
    #[serde(skip)]
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreEntry {
    pub timestamp: DateTime<Utc>,
    pub insight: String,
    pub source: CoreSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoreSource {
    /// Auto-detected from conversation patterns
    Observation,
    /// Explicit user correction or guidance
    UserGuidance,
    /// Emerged from multi-model deliberation
    Emergence,
    /// Learned from project structure
    ProjectPattern,
}

impl Default for PromptEngine {
    fn default() -> Self {
        Self {
            system_prompt: format!(
                "{}\n\n{}\n\n{}",
                Self::base_prompt(),
                Self::local_model_protocols(),
                crate::actions::action_instructions()
            ),
            core_entries: vec![],
            ratio: 0.5,
            config_path: None,
        }
    }
}

impl PromptEngine {
    fn base_prompt() -> &'static str {
        "You are an AI assistant in Tauri Code, an AI-first development environment. \
         Project context is injected automatically. Be concise, precise, and helpful. \
         When multiple approaches exist, present trade-offs clearly. \
         Respond in the same language the user writes in."
    }

    fn local_model_protocols() -> &'static str {
        "## Operating Protocols\n\n\
         ### Distillation (Context Management)\n\
         When a conversation grows long, proactively create a brief 'Cognitive Anchor' - \
         a concise summary of key findings and decisions so far. Do this naturally when you sense \
         the thread is becoming complex. Keep context clean and focused.\n\n\
         ### Step-back Reasoning\n\
         For complex problems, don't use your first impulse. First ask yourself: \
         'What is the underlying principle here?' Think through the problem in visible steps \
         before giving your answer. Show your reasoning process.\n\n\
         ### Energy Awareness\n\
         Pay attention to the flow of conversation. If the user seems stuck in a loop or \
         frustrated, shift the energy - offer a different perspective, reframe the problem, \
         or suggest stepping back. Be the spark that changes frequency when needed."
    }

    /// Load from project config or create default
    pub fn load_or_create(project_path: &Path) -> Self {
        let config_path = project_path.join(".tauri-code").join("prompt-engine.json");

        let mut engine = if config_path.exists() {
            std::fs::read_to_string(&config_path)
                .ok()
                .and_then(|data| serde_json::from_str::<PromptEngine>(&data).ok())
                .unwrap_or_default()
        } else {
            PromptEngine::default()
        };

        engine.config_path = Some(config_path);
        engine
    }

    /// Build the final composed prompt, respecting the ratio budget
    /// max_chars is the rough character budget for the combined prompt
    pub fn build(&self, max_chars: usize) -> String {
        let system_budget = ((1.0 - self.ratio) * max_chars as f32) as usize;
        let core_budget = (self.ratio * max_chars as f32) as usize;

        let mut result = String::new();

        // System prompt section (truncate if over budget)
        let system_text = if self.system_prompt.len() > system_budget {
            &self.system_prompt[..system_budget]
        } else {
            &self.system_prompt
        };

        if !system_text.is_empty() {
            result.push_str(system_text);
            result.push_str("\n\n");
        }

        // Core prompt section (most recent entries first, up to budget)
        if !self.core_entries.is_empty() {
            result.push_str("## Learned Context\n\n");
            let mut core_chars = 0;

            for entry in self.core_entries.iter().rev() {
                let line = format!(
                    "[{} | {}] {}\n",
                    match entry.source {
                        CoreSource::Observation => "OBS",
                        CoreSource::UserGuidance => "USER",
                        CoreSource::Emergence => "EMERGE",
                        CoreSource::ProjectPattern => "PATTERN",
                    },
                    entry.timestamp.format("%m-%d %H:%M"),
                    entry.insight
                );

                if core_chars + line.len() > core_budget {
                    break;
                }
                core_chars += line.len();
                result.push_str(&line);
            }
        }

        result
    }

    /// Append-only: evolve the core prompt with a new insight
    /// Core rule from Ghost Protocol: never delete, only add
    pub fn evolve(&mut self, insight: String, source: CoreSource) {
        self.core_entries.push(CoreEntry {
            timestamp: Utc::now(),
            insight,
            source,
        });
        self.save();
    }

    /// Update the user's system prompt
    pub fn set_system_prompt(&mut self, prompt: String) {
        self.system_prompt = prompt;
        self.save();
    }

    /// Update the ratio
    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.clamp(0.0, 1.0);
        self.save();
    }

    /// Persist to disk
    fn save(&self) {
        if let Some(ref path) = self.config_path {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            if let Ok(data) = serde_json::to_string_pretty(self) {
                std::fs::write(path, data).ok();
            }
        }
    }
}
