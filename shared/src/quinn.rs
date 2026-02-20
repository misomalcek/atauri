use serde::{Deserialize, Serialize};

/// Codex Guardian (Quinn) - The reality validator
///
/// "Trust Nothing. Prove it." - Quinn's Prime Directive
///
/// Quinn observes AI outputs, detects quality signals, and feeds back
/// into the system. Not a healer that patches cosmetically - an inquisitor
/// that exposes truth.
///
/// Dual purpose:
/// - In Tauri Code: validates AI responses in the chat pipeline
/// - As standalone agent: adversarial QA for any development session
///
/// Based on the Triad: Planner (user intent) → Generator (AI output) → Guardian (Quinn validates)

/// Quality signals Quinn detects in AI responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardianReport {
    /// Overall trust score (0.0 = suspicious, 1.0 = solid)
    pub trust: f32,
    /// Individual signal results
    pub signals: Vec<Signal>,
    /// Suggested feedback for the Prompt Engine (if any)
    pub feedback: Option<String>,
    /// Did this response need intervention?
    pub needs_attention: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

/// Quinn's core: evaluate an AI response against the user's request
pub fn evaluate(user_message: &str, ai_response: &str, is_local: bool) -> GuardianReport {
    let mut signals = Vec::new();

    // Signal 1: Substance check - is the response meaningfully sized relative to the question?
    let ratio = if user_message.len() > 0 {
        ai_response.len() as f32 / user_message.len() as f32
    } else {
        1.0
    };
    let substance_ok = ai_response.len() > 20 && ratio > 0.3;
    signals.push(Signal {
        name: "substance".into(),
        passed: substance_ok,
        detail: format!("response {} chars, ratio {:.1}x", ai_response.len(), ratio),
    });

    // Signal 2: Hallucination markers - phrases that indicate fabrication
    let hallucination_phrases = [
        "as an ai language model",
        "i cannot actually",
        "i don't have access to",
        "i apologize, but",
        "i'm not able to",
    ];
    let lower = ai_response.to_lowercase();
    let hallucination_count = hallucination_phrases
        .iter()
        .filter(|p| lower.contains(*p))
        .count();
    signals.push(Signal {
        name: "hallucination_markers".into(),
        passed: hallucination_count == 0,
        detail: format!("{} deflection phrases detected", hallucination_count),
    });

    // Signal 3: Code block integrity - if response has code blocks, are they closed?
    let open_blocks = ai_response.matches("```").count();
    let code_integrity = open_blocks % 2 == 0;
    signals.push(Signal {
        name: "code_integrity".into(),
        passed: code_integrity,
        detail: if code_integrity {
            format!("{} code fences (balanced)", open_blocks)
        } else {
            format!("{} code fences (UNCLOSED)", open_blocks)
        },
    });

    // Signal 4: Action block validity - if there are action blocks, are they parseable?
    let action_count = ai_response.matches("```action:").count();
    let actions_valid = if action_count > 0 {
        let parsed = crate::actions::parse_actions(ai_response);
        parsed.len() == action_count
    } else {
        true
    };
    signals.push(Signal {
        name: "action_validity".into(),
        passed: actions_valid,
        detail: if action_count > 0 {
            format!("{} action blocks, {} parsed", action_count, if actions_valid { "all valid" } else { "PARSE ERRORS" })
        } else {
            "no action blocks".into()
        },
    });

    // Signal 5: Repetition detection - repeated sentences suggest loop/stuck model
    let repetition = detect_repetition(ai_response);
    signals.push(Signal {
        name: "repetition".into(),
        passed: !repetition.is_repetitive,
        detail: if repetition.is_repetitive {
            format!("repetitive pattern: {} repeated segments", repetition.count)
        } else {
            "no repetition detected".into()
        },
    });

    // Signal 6: Language coherence - if user writes in Slovak, response should not be English-only
    // (and vice versa) - basic check
    let user_has_diacritics = user_message.chars().any(|c| "áäčďéíľňóôŕšťúýžÁÄČĎÉÍĽŇÓÔŔŠŤÚÝŽ".contains(c));
    let response_has_diacritics = ai_response.chars().any(|c| "áäčďéíľňóôŕšťúýžÁÄČĎÉÍĽŇÓÔŔŠŤÚÝŽ".contains(c));
    let lang_match = if user_has_diacritics {
        // User writes Slovak - response should have some diacritics too (unless very short/code)
        response_has_diacritics || ai_response.len() < 100 || ai_response.matches("```").count() > 0
    } else {
        true // English user, any response language is fine
    };
    signals.push(Signal {
        name: "language_match".into(),
        passed: lang_match,
        detail: if !lang_match {
            "user wrote Slovak but response is English-only".into()
        } else {
            "language consistent".into()
        },
    });

    // Signal 7: Local model specific - empty or error responses
    if is_local {
        let is_error = ai_response.trim().is_empty()
            || lower.starts_with("error")
            || lower.starts_with("{\"error")
            || ai_response.trim().len() < 5;
        signals.push(Signal {
            name: "local_health".into(),
            passed: !is_error,
            detail: if is_error {
                "local model returned empty/error response".into()
            } else {
                "local model responded normally".into()
            },
        });
    }

    // Calculate trust score
    // Substance is a gate: if the response has no substance, trust caps at 0.3
    let passed = signals.iter().filter(|s| s.passed).count();
    let total = signals.len();
    let mut trust = passed as f32 / total as f32;
    if !substance_ok {
        trust = trust.min(0.3);
    }

    let needs_attention = trust < 0.7;

    // Generate feedback for Prompt Engine if something's off
    let feedback = if needs_attention {
        let failed: Vec<&str> = signals.iter().filter(|s| !s.passed).map(|s| s.name.as_str()).collect();
        Some(format!(
            "Quinn detected quality issues: {}. Trust: {:.0}%",
            failed.join(", "),
            trust * 100.0
        ))
    } else {
        None
    };

    GuardianReport {
        trust,
        signals,
        feedback,
        needs_attention,
    }
}

/// Detect repetitive patterns in text (sign of stuck/looping model)
struct RepetitionResult {
    is_repetitive: bool,
    count: usize,
}

fn detect_repetition(text: &str) -> RepetitionResult {
    let sentences: Vec<&str> = text
        .split(|c: char| c == '.' || c == '\n')
        .map(|s| s.trim())
        .filter(|s| s.len() > 20) // Only check substantial segments
        .collect();

    if sentences.len() < 4 {
        return RepetitionResult { is_repetitive: false, count: 0 };
    }

    // Check for exact duplicate sentences
    let mut seen = std::collections::HashSet::new();
    let mut duplicates = 0;
    for s in &sentences {
        let normalized: String = s.to_lowercase().chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
        if !seen.insert(normalized) {
            duplicates += 1;
        }
    }

    // More than 30% duplicates = repetitive
    let threshold = (sentences.len() as f32 * 0.3) as usize;
    RepetitionResult {
        is_repetitive: duplicates > threshold.max(2),
        count: duplicates,
    }
}

/// Format a GuardianReport as a compact status line for the UI
pub fn format_status(report: &GuardianReport) -> String {
    let icon = if report.trust >= 0.85 {
        "SOLID"
    } else if report.trust >= 0.7 {
        "OK"
    } else {
        "ALERT"
    };

    let failed: Vec<&str> = report.signals.iter()
        .filter(|s| !s.passed)
        .map(|s| s.name.as_str())
        .collect();

    if failed.is_empty() {
        format!("Quinn: {} ({:.0}%)", icon, report.trust * 100.0)
    } else {
        format!("Quinn: {} ({:.0}%) [{}]", icon, report.trust * 100.0, failed.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_good_response() {
        let report = evaluate(
            "How do I create a new file in Rust?",
            "You can use `std::fs::write()` to create a file:\n\n```rust\nstd::fs::write(\"hello.txt\", \"Hello world\").unwrap();\n```\n\nThis creates the file and writes the content in one call.",
            false,
        );
        assert!(report.trust >= 0.8);
        assert!(!report.needs_attention);
    }

    #[test]
    fn test_empty_response() {
        let report = evaluate("Tell me about Rust", "", true);
        assert!(report.trust < 0.7);
        assert!(report.needs_attention);
    }

    #[test]
    fn test_hallucination_markers() {
        let report = evaluate(
            "Write a function",
            "As an AI language model, I cannot actually write code. I apologize, but I'm not able to help with that.",
            false,
        );
        assert!(!report.signals.iter().find(|s| s.name == "hallucination_markers").unwrap().passed);
    }

    #[test]
    fn test_unclosed_code_block() {
        let report = evaluate(
            "Show me code",
            "Here's the code:\n\n```rust\nfn main() {\n    println!(\"hello\");\n}\n\nThat should work!",
            false,
        );
        assert!(!report.signals.iter().find(|s| s.name == "code_integrity").unwrap().passed);
    }
}
