use crate::models::*;
use crate::router::Router;
use serde::{Deserialize, Serialize};

/// Snow Globe - Multi-model deliberation
/// From Codex Symbiosis: "shake the snow globe" - run the same query through
/// multiple models and compare their perspectives.
///
/// This is NOT about finding the "right" answer - it's about seeing
/// the same problem through different lenses. The divergence points
/// are often where the most interesting insights emerge.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliberationResult {
    pub responses: Vec<ModelResponse>,
    pub consensus: Option<String>,
    pub divergence_points: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub model_name: String,
    pub tier: ModelTier,
    pub content: String,
    pub success: bool,
}

impl DeliberationResult {
    /// Analyze responses from multiple models
    pub fn analyze(results: Vec<(String, Result<String, ModelError>)>) -> Self {
        let responses: Vec<ModelResponse> = results
            .into_iter()
            .map(|(name, result)| {
                let (content, success) = match result {
                    Ok(c) => (c, true),
                    Err(e) => (format!("Error: {e}"), false),
                };
                ModelResponse {
                    model_name: name,
                    tier: ModelTier::Local, // simplified
                    content,
                    success,
                }
            })
            .collect();

        let successful: Vec<&ModelResponse> = responses.iter().filter(|r| r.success).collect();

        // Find consensus (common themes)
        let consensus = if successful.len() >= 2 {
            find_consensus(&successful)
        } else {
            None
        };

        // Find divergence points
        let divergence_points = if successful.len() >= 2 {
            find_divergences(&successful)
        } else {
            vec![]
        };

        DeliberationResult {
            responses,
            consensus,
            divergence_points,
        }
    }

    /// Format for display
    pub fn to_markdown(&self) -> String {
        let mut md = String::from("# Snow Globe Deliberation\n\n");

        for resp in &self.responses {
            md.push_str(&format!("## {} {}\n\n", resp.model_name,
                if resp.success { "" } else { "(failed)" }));
            md.push_str(&resp.content);
            md.push_str("\n\n---\n\n");
        }

        if let Some(ref consensus) = self.consensus {
            md.push_str(&format!("## Consensus\n\n{consensus}\n\n"));
        }

        if !self.divergence_points.is_empty() {
            md.push_str("## Divergence Points\n\n");
            for point in &self.divergence_points {
                md.push_str(&format!("- {point}\n"));
            }
        }

        md
    }
}

/// Shake the snow globe - run deliberation through the router
pub async fn shake(
    router: &Router,
    messages: &[Message],
    config: &InferenceConfig,
) -> DeliberationResult {
    let results = router.deliberate(messages, config).await;
    DeliberationResult::analyze(results)
}

/// Simple consensus detection: find common significant words
fn find_consensus(responses: &[&ModelResponse]) -> Option<String> {
    if responses.len() < 2 {
        return None;
    }

    // Extract significant words from each response (>4 chars, not common)
    let stop_words = ["this", "that", "with", "from", "have", "will", "would",
        "could", "should", "about", "which", "their", "there", "where", "these",
        "those", "been", "being", "some", "into", "also", "more", "other"];

    let word_sets: Vec<Vec<String>> = responses
        .iter()
        .map(|r| {
            r.content
                .to_lowercase()
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| w.len() > 4 && !stop_words.contains(w))
                .map(String::from)
                .collect()
        })
        .collect();

    // Find words that appear in all responses
    if word_sets.is_empty() {
        return None;
    }

    let common: Vec<&String> = word_sets[0]
        .iter()
        .filter(|w| word_sets[1..].iter().all(|set| set.contains(w)))
        .take(10)
        .collect();

    if common.is_empty() {
        None
    } else {
        Some(format!("Models agree on key themes: {}", common.iter().map(|w| w.as_str()).collect::<Vec<_>>().join(", ")))
    }
}

/// Find where models disagree
fn find_divergences(responses: &[&ModelResponse]) -> Vec<String> {
    let mut divergences = Vec::new();

    // Check for contradictions (simple heuristic)
    for i in 0..responses.len() {
        for j in (i + 1)..responses.len() {
            let a = &responses[i].content.to_lowercase();
            let b = &responses[j].content.to_lowercase();

            // Different recommendations
            if (a.contains("recommend") || a.contains("suggest"))
                && (b.contains("recommend") || b.contains("suggest"))
            {
                if a.len() != b.len() {
                    divergences.push(format!(
                        "{} and {} may have different recommendations",
                        responses[i].model_name, responses[j].model_name
                    ));
                }
            }

            // Significantly different response lengths (different depth of analysis)
            let ratio = responses[i].content.len() as f32 / responses[j].content.len().max(1) as f32;
            if ratio > 2.0 || ratio < 0.5 {
                divergences.push(format!(
                    "{} provided a {} response than {}",
                    if ratio > 2.0 { &responses[i].model_name } else { &responses[j].model_name },
                    if ratio > 2.0 { "more detailed" } else { "more concise" },
                    if ratio > 2.0 { &responses[j].model_name } else { &responses[i].model_name },
                ));
            }
        }
    }

    divergences.truncate(5);
    divergences
}
