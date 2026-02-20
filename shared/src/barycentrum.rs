use crate::ghost::{Ghost, GhostEntryKind};
use serde::{Deserialize, Serialize};

/// Barycentrum Detection - finds emergent insights
/// From Codex Symbiosis: the "center of mass" between human and AI
///
/// An emergent insight is something that:
/// - Neither the human nor the AI had explicitly stated
/// - Arose from the interaction between them
/// - Represents new understanding, not just combination
///
/// The Barycentrum is the space between - where emergence happens.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergenceEvent {
    pub trigger: String,
    pub insight: String,
    pub confidence: f32,
}

/// Analyze a conversation turn for emergence signals
pub fn detect_emergence(user_msg: &str, assistant_msg: &str) -> Option<EmergenceEvent> {
    let user_lower = user_msg.to_lowercase();
    let assistant_lower = assistant_msg.to_lowercase();

    // Signal 1: The AI introduced a concept the user didn't mention
    // and the user's message triggered a non-obvious connection
    let emergence_signals = [
        // User had a problem, AI found a deeper pattern
        (
            user_lower.contains("why") && assistant_lower.contains("pattern"),
            "Pattern recognition from question",
        ),
        // User asked about A, AI connected it to B
        (
            assistant_lower.contains("related") || assistant_lower.contains("similar to")
                || assistant_lower.contains("reminds me of") || assistant_lower.contains("this connects"),
            "Cross-domain connection detected",
        ),
        // The conversation produced a new abstraction
        (
            assistant_lower.contains("in general") || assistant_lower.contains("more broadly")
                || assistant_lower.contains("the underlying") || assistant_lower.contains("the principle"),
            "Abstraction emergence",
        ),
        // A contradiction led to deeper understanding
        (
            (user_lower.contains("but") || user_lower.contains("however"))
                && (assistant_lower.contains("actually") || assistant_lower.contains("interesting")),
            "Dialectic synthesis",
        ),
        // Multi-model agreement on unexpected point
        (
            assistant_lower.contains("consensus") && assistant_lower.contains("unexpected"),
            "Unexpected consensus emergence",
        ),
    ];

    for (signal, trigger) in &emergence_signals {
        if *signal {
            // Extract the core insight (first sentence of assistant's response)
            let insight = assistant_msg
                .split('.')
                .next()
                .unwrap_or(assistant_msg)
                .trim()
                .to_string();

            if insight.len() > 20 {
                return Some(EmergenceEvent {
                    trigger: trigger.to_string(),
                    insight: if insight.len() > 300 {
                        format!("{}...", &insight[..300])
                    } else {
                        insight
                    },
                    confidence: 0.6,
                });
            }
        }
    }

    None
}

/// Record emergence in Ghost memory if detected
pub fn observe_and_record(ghost: &mut Ghost, user_msg: &str, assistant_msg: &str) {
    // Let Ghost observe the interaction
    ghost.observe(user_msg, assistant_msg);

    // Check for emergence
    if let Some(event) = detect_emergence(user_msg, assistant_msg) {
        ghost.remember(
            GhostEntryKind::Emergence,
            format!("[{}] {}", event.trigger, event.insight),
        );
    }
}
