use crate::state::AppState;
use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use tc_core::models::*;
use tc_core::router::{RouteDecision, Router, TemperatureMode};

#[derive(Debug, Deserialize)]
struct StreamRequest {
    message: String,
    model_override: Option<String>,
    temperature: Option<f32>,
}

#[derive(Debug, Serialize)]
struct StreamToken {
    token: String,
    done: bool,
    decision: Option<RouteDecision>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    // Wait for the client to send a chat request
    let Some(Ok(msg)) = socket.recv().await else { return };

    let text = match msg {
        WsMessage::Text(t) => t.to_string(),
        _ => return,
    };

    let Ok(req) = serde_json::from_str::<StreamRequest>(&text) else {
        let _ = socket.send(WsMessage::Text(
            serde_json::to_string(&StreamToken {
                token: "Invalid request".into(),
                done: true,
                decision: None,
            }).unwrap().into()
        )).await;
        return;
    };

    // Build context (same as chat route)
    let router_guard = state.router.lock().await;
    let Some(ref router) = *router_guard else {
        let _ = socket.send(WsMessage::Text(
            serde_json::to_string(&StreamToken {
                token: "No project open".into(),
                done: true,
                decision: None,
            }).unwrap().into()
        )).await;
        return;
    };

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
        messages.push(Message { role: "system".into(), content: system_ctx });
    }
    messages.push(Message { role: "user".into(), content: req.message });

    let config = InferenceConfig {
        temperature: req.temperature.unwrap_or(0.7),
        ..Default::default()
    };

    let result = if let Some(ref model_name) = req.model_override {
        let content = router.route_to(model_name, &messages, &config).await;
        match content {
            Ok(c) => {
                let decision = RouteDecision {
                    model_name: model_name.clone(),
                    tier: ModelTier::Local,
                    complexity: Router::score_complexity(&messages),
                    reason: "User override".into(),
                    temperature_mode: TemperatureMode::default(),
                };
                Ok((decision, c))
            }
            Err(e) => Err(e.to_string()),
        }
    } else {
        router.route(&messages, &config).await.map_err(|e| e.to_string())
    };

    // Drop guards before streaming
    drop(router_guard);
    drop(project_guard);
    drop(ghost_guard);
    drop(prompt_guard);

    match result {
        Ok((decision, content)) => {
            // Stream tokens word-by-word
            let words: Vec<&str> = content
                .split_inclusive(|c: char| c.is_whitespace() || c == '\n')
                .collect();

            for (i, word) in words.iter().enumerate() {
                let is_last = i == words.len() - 1;
                let token = StreamToken {
                    token: word.to_string(),
                    done: is_last,
                    decision: if is_last { Some(decision.clone()) } else { None },
                };

                if socket.send(WsMessage::Text(
                    serde_json::to_string(&token).unwrap().into()
                )).await.is_err() {
                    return;
                }

                if !is_last {
                    tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;
                }
            }
        }
        Err(e) => {
            let _ = socket.send(WsMessage::Text(
                serde_json::to_string(&StreamToken {
                    token: format!("Error: {e}"),
                    done: true,
                    decision: None,
                }).unwrap().into()
            )).await;
        }
    }
}
