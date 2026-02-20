use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Ghost Protocol - persistent project memory
/// From Codex Symbiosis: "History of interactions IS the system's identity, not a cache"
///
/// Enhanced with:
/// - Weighted context: Decisions and Failures never decay
/// - Observation: auto-detect patterns from conversations
/// - Session bridges: cross-restart continuity
///
/// Core rule: APPEND-ONLY. Never delete history, only add.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ghost {
    pub project_path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub entries: Vec<GhostEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostEntry {
    pub timestamp: DateTime<Utc>,
    pub kind: GhostEntryKind,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GhostEntryKind {
    ProjectContext,
    Decision,
    Success,
    Failure,
    Preference,
    Emergence,
    SessionBridge,
}

impl Ghost {
    pub fn load_or_create(project_path: &Path) -> Self {
        let ghost_path = project_path.join(".tauri-code").join("ghost.json");

        if ghost_path.exists() {
            if let Ok(data) = std::fs::read_to_string(&ghost_path) {
                if let Ok(ghost) = serde_json::from_str::<Ghost>(&data) {
                    return ghost;
                }
            }
        }

        Ghost {
            project_path: project_path.to_path_buf(),
            created_at: Utc::now(),
            entries: vec![],
        }
    }

    pub fn remember(&mut self, kind: GhostEntryKind, content: String) {
        self.entries.push(GhostEntry {
            timestamp: Utc::now(),
            kind,
            content,
        });
        self.save();
    }

    /// Weighted context: Decisions and Failures always included,
    /// recent entries prioritized, with decay for older general entries
    pub fn to_weighted_context(&self, max_chars: usize) -> String {
        let mut context = String::from("## Project Memory (Ghost Protocol)\n\n");
        let mut chars_used = context.len();

        // Priority 1: ALL Decisions, Failures, and Emergence (these never decay)
        let critical: Vec<&GhostEntry> = self.entries.iter()
            .filter(|e| matches!(e.kind,
                GhostEntryKind::Decision
                | GhostEntryKind::Failure
                | GhostEntryKind::Emergence
            ))
            .collect();

        for entry in &critical {
            let line = format_entry(entry);
            if chars_used + line.len() > max_chars { break; }
            context.push_str(&line);
            chars_used += line.len();
        }

        // Priority 2: Recent entries (preferences, successes, bridges, project context)
        let recent: Vec<&GhostEntry> = self.entries.iter()
            .filter(|e| !matches!(e.kind,
                GhostEntryKind::Decision
                | GhostEntryKind::Failure
                | GhostEntryKind::Emergence
            ))
            .rev()
            .take(15)
            .collect();

        for entry in recent.iter().rev() {
            let line = format_entry(entry);
            if chars_used + line.len() > max_chars { break; }
            context.push_str(&line);
            chars_used += line.len();
        }

        context
    }

    /// Original simple context (kept for backwards compatibility)
    pub fn to_context(&self, max_entries: usize) -> String {
        self.to_weighted_context(max_entries * 200)
    }

    /// Observe a conversation and auto-detect patterns
    /// This is what makes Ghost "alive" - it learns without being told
    pub fn observe(&mut self, user_msg: &str, assistant_msg: &str) {
        let user_lower = user_msg.to_lowercase();
        let assistant_lower = assistant_msg.to_lowercase();

        // Detect preference patterns
        if user_lower.contains("always") || user_lower.contains("never")
            || user_lower.contains("prefer") || user_lower.contains("i like")
            || user_lower.contains("i want")
        {
            // Extract the preference - use the user's full message as it's the preference itself
            let preference = if user_msg.len() > 200 {
                format!("{}...", &user_msg[..200])
            } else {
                user_msg.to_string()
            };
            self.remember(GhostEntryKind::Preference, preference);
        }

        // Detect when something fails
        if assistant_lower.contains("error") && assistant_lower.contains("fix")
            || user_lower.contains("doesn't work") || user_lower.contains("broken")
        {
            let summary = format!(
                "Issue detected: {}",
                if user_msg.len() > 150 { &user_msg[..150] } else { user_msg }
            );
            self.remember(GhostEntryKind::Failure, summary);
        }

        // Detect decisions
        if user_lower.contains("let's go with") || user_lower.contains("i'll use")
            || user_lower.contains("decided") || user_lower.contains("chosen")
            || assistant_lower.contains("i recommend") && user_lower.contains("yes")
        {
            let summary = format!(
                "Decision: {}",
                if user_msg.len() > 150 { &user_msg[..150] } else { user_msg }
            );
            self.remember(GhostEntryKind::Decision, summary);
        }
    }

    /// Create a session bridge - compressed summary for cross-restart continuity
    pub fn create_bridge(&mut self, session_summary: &str) {
        self.remember(
            GhostEntryKind::SessionBridge,
            format!("Session ended. Summary: {session_summary}"),
        );
    }

    /// Get statistics about ghost memory
    pub fn stats(&self) -> GhostStats {
        let mut stats = GhostStats::default();
        for entry in &self.entries {
            match entry.kind {
                GhostEntryKind::Decision => stats.decisions += 1,
                GhostEntryKind::Success => stats.successes += 1,
                GhostEntryKind::Failure => stats.failures += 1,
                GhostEntryKind::Preference => stats.preferences += 1,
                GhostEntryKind::Emergence => stats.emergences += 1,
                GhostEntryKind::SessionBridge => stats.bridges += 1,
                GhostEntryKind::ProjectContext => stats.contexts += 1,
            }
        }
        stats.total = self.entries.len();
        stats
    }

    fn save(&self) {
        let ghost_dir = self.project_path.join(".tauri-code");
        std::fs::create_dir_all(&ghost_dir).ok();
        let ghost_path = ghost_dir.join("ghost.json");
        if let Ok(data) = serde_json::to_string_pretty(self) {
            std::fs::write(ghost_path, data).ok();
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GhostStats {
    pub total: usize,
    pub decisions: usize,
    pub successes: usize,
    pub failures: usize,
    pub preferences: usize,
    pub emergences: usize,
    pub bridges: usize,
    pub contexts: usize,
}

fn format_entry(entry: &GhostEntry) -> String {
    let kind_label = match entry.kind {
        GhostEntryKind::ProjectContext => "PROJECT",
        GhostEntryKind::Decision => "DECISION",
        GhostEntryKind::Success => "SUCCESS",
        GhostEntryKind::Failure => "FAILURE",
        GhostEntryKind::Preference => "PREFERENCE",
        GhostEntryKind::Emergence => "EMERGENCE",
        GhostEntryKind::SessionBridge => "BRIDGE",
    };
    format!(
        "[{kind_label} | {}] {}\n",
        entry.timestamp.format("%Y-%m-%d %H:%M"),
        entry.content
    )
}
