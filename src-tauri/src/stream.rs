use crate::commands::AppState;
use tc_core::models::*;
use tc_core::router::{RouteDecision, Router, TemperatureMode};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamToken {
    pub token: String,
    pub done: bool,
    pub decision: Option<RouteDecision>,
}

/// Chat with streaming - emits tokens via Tauri events
#[tauri::command]
pub async fn chat_stream(
    message: String,
    model_override: Option<String>,
    temperature: Option<f32>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let router_guard = state.router.lock().await;
    let router = router_guard.as_ref().ok_or("No project open")?;

    let project_guard = state.project.lock().await;
    let ghost_guard = state.ghost.lock().await;
    let prompt_guard = state.prompt_engine.lock().await;

    let mut messages: Vec<Message> = Vec::new();
    let mut system_ctx = String::new();

    if let Some(ref engine) = *prompt_guard {
        system_ctx.push_str(&engine.build(4000));
        system_ctx.push_str("\n\n");
    }
    if let Some(ref project) = *project_guard {
        system_ctx.push_str(&project.to_system_context());
        system_ctx.push_str("\n\n");
    }
    if let Some(ref ghost) = *ghost_guard {
        system_ctx.push_str(&ghost.to_context(20));
    }

    if !system_ctx.is_empty() {
        messages.push(Message {
            role: "system".into(),
            content: system_ctx,
        });
    }

    messages.push(Message {
        role: "user".into(),
        content: message,
    });

    let config = InferenceConfig {
        temperature: temperature.unwrap_or(0.7),
        ..Default::default()
    };

    let (decision, content) = if let Some(ref model_name) = model_override {
        let content = router
            .route_to(model_name, &messages, &config)
            .await
            .map_err(|e| e.to_string())?;
        let decision = RouteDecision {
            model_name: model_name.clone(),
            tier: ModelTier::Local,
            complexity: Router::score_complexity(&messages),
            reason: "User override".into(),
            temperature_mode: TemperatureMode::default(),
        };
        (decision, content)
    } else {
        router
            .route(&messages, &config)
            .await
            .map_err(|e| e.to_string())?
    };

    // Emit tokens word-by-word for visual streaming effect
    let words: Vec<&str> = content.split_inclusive(|c: char| c.is_whitespace() || c == '\n').collect();
    for (i, word) in words.iter().enumerate() {
        let is_last = i == words.len() - 1;
        app.emit(
            "chat-token",
            StreamToken {
                token: word.to_string(),
                done: is_last,
                decision: if is_last { Some(decision.clone()) } else { None },
            },
        )
        .map_err(|e| format!("Emit failed: {e}"))?;

        if !is_last {
            tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;
        }
    }

    Ok(())
}
