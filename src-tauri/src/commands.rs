use tc_core::context::ProjectContext;
use tc_core::ghost::{Ghost, GhostEntryKind};
use tc_core::models::*;
use tc_core::prompt::{CoreSource, PromptEngine};
use tc_core::qdrant::KnowledgeBase;
use tc_core::router::{RouteDecision, Router};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

/// Shared application state - accessible from all Tauri commands
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
    // Legacy field - kept for backward compat with old configs
    #[serde(default = "default_threshold")]
    pub complexity_threshold: f32,
    pub default_model: Option<String>,
    // Weaver Spectrum fields
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
        let home = dirs_or_default();
        home.join(".tauri-code").join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|data| serde_json::from_str::<AppConfig>(&data).ok())
                .unwrap_or_default()
        } else {
            AppConfig::default()
        }
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

fn dirs_or_default() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub model_override: Option<String>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub decision: RouteDecision,
    /// Quinn's quality assessment of the AI response
    pub guardian: Option<tc_core::quinn::GuardianReport>,
}

/// Open a project - scans structure, loads ghost, loads prompt engine, initializes router
#[tauri::command]
pub async fn open_project(path: String, state: State<'_, AppState>) -> Result<ProjectContext, String> {
    let project_path = PathBuf::from(&path);

    let ctx = ProjectContext::scan(&project_path);
    let ghost = Ghost::load_or_create(&project_path);
    let prompt_engine = PromptEngine::load_or_create(&project_path);

    let config = state.config.lock().await;
    let local_models = local::LmStudioModel::discover(config.lm_studio_port).await;

    let mut models: Vec<Arc<dyn AiModel>> = Vec::new();

    for model_info in &local_models {
        models.push(Arc::new(local::LmStudioModel::new(
            &model_info.id,
            &model_info.name,
            config.lm_studio_port,
        )));
    }

    if let Some(ref api_key) = config.claude_api_key {
        let key = api_key.trim();
        if !key.is_empty() {
            models.push(Arc::new(cloud::ClaudeModel::haiku(key)));
            models.push(Arc::new(cloud::ClaudeModel::sonnet(key)));
            models.push(Arc::new(cloud::ClaudeModel::opus(key)));
        }
    }

    let router = Router::new_with_config(models, config.to_routing_config());

    *state.router.lock().await = Some(router);
    *state.ghost.lock().await = Some(ghost);
    *state.prompt_engine.lock().await = Some(prompt_engine);
    *state.project.lock().await = Some(ctx.clone());

    // Clear conversation history on new project
    state.conversation.lock().await.clear();

    // Auto-connect Qdrant if configured
    let qdrant_url = config.qdrant_url.as_deref().unwrap_or("http://localhost:6333").to_string();
    let embedding_port = config.lm_studio_port;
    drop(config);
    if let Ok(kb) = KnowledgeBase::connect(&qdrant_url, embedding_port).await {
        // Auto-ensure a per-project collection exists
        let collection_name = format!("project-{}", ctx.name.to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
            .collect::<String>());
        // Create collection if it doesn't exist (non-blocking, don't fail project open)
        let _ = kb.ensure_project_collection(&collection_name).await;
        *state.knowledge_base.lock().await = Some(kb);
    }

    Ok(ctx)
}

/// Send a chat message - routes to best model automatically
#[tauri::command]
pub async fn chat(request: ChatRequest, state: State<'_, AppState>) -> Result<ChatResponse, String> {
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

    // Auto-RAG: search knowledge base for relevant context
    {
        let kb_guard = state.knowledge_base.lock().await;
        if let Some(ref kb) = *kb_guard {
            let collections = kb.list_collections().await.unwrap_or_default();
            if !collections.is_empty() {
                let mut rag_results = Vec::new();
                for coll_info in &collections {
                    if let Ok(results) = kb.search(&request.message, &coll_info.name, 3, None).await {
                        for r in results {
                            if r.score > 0.3 || !rag_results.is_empty() {
                                rag_results.push(r);
                            }
                        }
                    }
                }
                rag_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
                rag_results.truncate(5);

                if !rag_results.is_empty() {
                    system_ctx.push_str("\n\n## Relevant Knowledge (from project knowledge base)\n\n");
                    for r in &rag_results {
                        let truncated: String = r.code_chunk.chars().take(500).collect();
                        system_ctx.push_str(&format!(
                            "**{}** (L{}-{}, relevance: {:.2})\n{}\n\n",
                            r.file_path, r.start_line, r.end_line, r.score,
                            truncated
                        ));
                    }
                }
            }
        }
    }

    if !system_ctx.is_empty() {
        messages.push(Message {
            role: "system".into(),
            content: system_ctx,
        });
    }

    // Include conversation history with smart compression for local models
    // If history > 10 messages: compress older ones into a digest + keep last 10 raw
    {
        let history = state.conversation.lock().await;
        if history.len() > 10 {
            let older = &history[..history.len() - 10];
            let recent = &history[history.len() - 10..];

            // Compress older messages into a conversation digest
            let digest = compress_history(older);
            messages.push(Message {
                role: "system".into(),
                content: format!("## Conversation History (compressed)\n{}", digest),
            });

            for msg in recent {
                messages.push(msg.clone());
            }
        } else {
            for msg in history.iter() {
                messages.push(msg.clone());
            }
        }
    }

    let user_message = request.message.clone();

    // Pre-route complexity check for local model nudges
    let complexity = Router::score_complexity(&messages);

    // CoT nudge: if task is non-trivial and will likely go to local model → prefix with step-by-step
    let final_message = if complexity > 0.4 && complexity < router.config.cloud_floor {
        format!("Premysli krok za krokom:\n\n{}", request.message)
    } else {
        request.message
    };

    // Vibe check: if last 3 user messages are very short, user might be stuck → nudge AI to shift
    {
        let history = state.conversation.lock().await;
        let recent_user: Vec<&Message> = history.iter().rev()
            .filter(|m| m.role == "user")
            .take(3)
            .collect();
        if recent_user.len() == 3 && recent_user.iter().all(|m| m.content.len() < 20) {
            messages.push(Message {
                role: "system".into(),
                content: "Používateľ odpovedá veľmi stručne. Skús zmeniť prístup - ponúkni inú perspektívu, zhrň čo máme, alebo polož konkrétnu otázku.".into(),
            });
        }
    }

    messages.push(Message {
        role: "user".into(),
        content: final_message,
    });

    let config = InferenceConfig {
        temperature: request.temperature.unwrap_or(0.7),
        ..Default::default()
    };

    let (decision, content) = if let Some(ref model_name) = request.model_override {
        let content = router
            .route_to(model_name, &messages, &config)
            .await
            .map_err(|e| e.to_string())?;
        let decision = RouteDecision {
            model_name: model_name.clone(),
            tier: ModelTier::Local,
            complexity: Router::score_complexity(&messages),
            reason: "User override".into(),
            temperature_mode: tc_core::router::TemperatureMode::default(),
        };
        (decision, content)
    } else {
        router
            .route(&messages, &config)
            .await
            .map_err(|e| e.to_string())?
    };

    drop(router_guard);
    drop(project_guard);
    drop(ghost_guard);
    drop(prompt_guard);

    // Quinn: evaluate the AI response
    let is_local = decision.tier == ModelTier::Local;
    let guardian_report = tc_core::quinn::evaluate(&user_message, &content, is_local);

    // If Quinn detects issues, feed back to Prompt Engine
    if let Some(ref feedback) = guardian_report.feedback {
        let mut engine_guard = state.prompt_engine.lock().await;
        if let Some(ref mut engine) = *engine_guard {
            engine.evolve(feedback.clone(), CoreSource::Observation);
        }
    }

    // Store conversation history
    {
        let mut history = state.conversation.lock().await;
        history.push(Message { role: "user".into(), content: user_message.clone() });
        history.push(Message { role: "assistant".into(), content: content.clone() });
        // Cap at 50 messages to prevent memory bloat
        if history.len() > 50 {
            let drain = history.len() - 50;
            history.drain(..drain);
        }
    }

    // Barycentrum: observe the interaction for emergence and patterns
    {
        let mut ghost_write = state.ghost.lock().await;
        if let Some(ref mut ghost) = *ghost_write {
            tc_core::barycentrum::observe_and_record(ghost, &user_message, &content);
        }
    }

    let guardian = if guardian_report.needs_attention { Some(guardian_report) } else { None };
    Ok(ChatResponse { content, decision, guardian })
}

/// Clear conversation history
#[tauri::command]
pub async fn clear_conversation(state: State<'_, AppState>) -> Result<(), String> {
    state.conversation.lock().await.clear();
    Ok(())
}

/// Discover available models (local + cloud)
#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    let config = state.config.lock().await;
    let mut models = local::LmStudioModel::discover(config.lm_studio_port).await;

    let has_claude_key = config.claude_api_key
        .as_deref()
        .map(|k| !k.is_empty())
        .unwrap_or(false);

    if has_claude_key {
        models.push(ModelInfo {
            id: "claude-haiku-4-5-20251001".into(),
            name: "Claude Haiku 4.5".into(),
            tier: ModelTier::Cloud,
            available: true,
        });
        models.push(ModelInfo {
            id: "claude-sonnet-4-6".into(),
            name: "Claude Sonnet 4.6".into(),
            tier: ModelTier::Cloud,
            available: true,
        });
        models.push(ModelInfo {
            id: "claude-opus-4-6".into(),
            name: "Claude Opus 4.6".into(),
            tier: ModelTier::Cloud,
            available: true,
        });
    }

    Ok(models)
}

/// Remember something in the ghost (from UI)
#[tauri::command]
pub async fn ghost_remember(
    kind: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut ghost_guard = state.ghost.lock().await;
    let ghost = ghost_guard.as_mut().ok_or("No project open")?;

    let entry_kind = match kind.as_str() {
        "decision" => GhostEntryKind::Decision,
        "success" => GhostEntryKind::Success,
        "failure" => GhostEntryKind::Failure,
        "preference" => GhostEntryKind::Preference,
        "emergence" => GhostEntryKind::Emergence,
        _ => GhostEntryKind::ProjectContext,
    };

    ghost.remember(entry_kind, content);
    Ok(())
}

/// Get project file tree
#[tauri::command]
pub async fn get_file_tree(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or("No project open")?;
    Ok(project.key_files.iter().map(|f| f.path.clone()).collect())
}

/// Read a file from the project
#[tauri::command]
pub async fn read_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read {path}: {e}"))
}

// ──────────────────────────────────────
// Settings + Prompt Engine commands
// ──────────────────────────────────────

#[tauri::command]
pub async fn save_settings(settings: AppConfig, state: State<'_, AppState>) -> Result<(), String> {
    settings.save();

    // Re-initialize router with new config (picks up new API keys, thresholds, etc.)
    let local_models = local::LmStudioModel::discover(settings.lm_studio_port).await;
    let mut models: Vec<Arc<dyn AiModel>> = Vec::new();
    for model_info in &local_models {
        models.push(Arc::new(local::LmStudioModel::new(
            &model_info.id,
            &model_info.name,
            settings.lm_studio_port,
        )));
    }
    if let Some(ref api_key) = settings.claude_api_key {
        let key = api_key.trim();
        if !key.is_empty() {
            models.push(Arc::new(cloud::ClaudeModel::haiku(key)));
            models.push(Arc::new(cloud::ClaudeModel::sonnet(key)));
            models.push(Arc::new(cloud::ClaudeModel::opus(key)));
        }
    }
    if !models.is_empty() {
        let router = Router::new_with_config(models, settings.to_routing_config());
        *state.router.lock().await = Some(router);
    }

    // Re-connect Qdrant if URL changed
    if let Some(ref url) = settings.qdrant_url {
        if let Ok(kb) = KnowledgeBase::connect(url, settings.lm_studio_port).await {
            *state.knowledge_base.lock().await = Some(kb);
        }
    }

    *state.config.lock().await = settings;
    Ok(())
}

#[tauri::command]
pub async fn load_settings(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let config = state.config.lock().await;
    Ok(config.clone())
}

#[tauri::command]
pub async fn update_system_prompt(prompt: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut engine_guard = state.prompt_engine.lock().await;
    let engine = engine_guard.as_mut().ok_or("No project open")?;
    engine.set_system_prompt(prompt);
    Ok(())
}

#[tauri::command]
pub async fn evolve_core_prompt(
    insight: String,
    source: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut engine_guard = state.prompt_engine.lock().await;
    let engine = engine_guard.as_mut().ok_or("No project open")?;

    let core_source = match source.as_str() {
        "observation" => CoreSource::Observation,
        "user" => CoreSource::UserGuidance,
        "emergence" => CoreSource::Emergence,
        "pattern" => CoreSource::ProjectPattern,
        _ => CoreSource::Observation,
    };

    engine.evolve(insight, core_source);
    Ok(())
}

#[tauri::command]
pub async fn set_prompt_ratio(ratio: f32, state: State<'_, AppState>) -> Result<(), String> {
    let mut engine_guard = state.prompt_engine.lock().await;
    let engine = engine_guard.as_mut().ok_or("No project open")?;
    engine.set_ratio(ratio);
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct PromptState {
    pub system_prompt: String,
    pub core_entries_count: usize,
    pub core_preview: String,
    pub ratio: f32,
}

#[tauri::command]
pub async fn get_prompt_state(state: State<'_, AppState>) -> Result<PromptState, String> {
    let engine_guard = state.prompt_engine.lock().await;
    let engine = engine_guard.as_ref().ok_or("No project open")?;

    let core_preview = engine
        .core_entries
        .iter()
        .rev()
        .take(5)
        .map(|e| e.insight.clone())
        .collect::<Vec<_>>()
        .join("\n");

    Ok(PromptState {
        system_prompt: engine.system_prompt.clone(),
        core_entries_count: engine.core_entries.len(),
        core_preview,
        ratio: engine.ratio,
    })
}

// ──────────────────────────────────────
// Qdrant Knowledge Base commands
// ──────────────────────────────────────

#[tauri::command]
pub async fn connect_qdrant(state: State<'_, AppState>) -> Result<String, String> {
    let config = state.config.lock().await;
    let url = config.qdrant_url.as_deref().unwrap_or("http://localhost:6333");
    let port = config.lm_studio_port;

    let kb = KnowledgeBase::connect(url, port).await?;
    *state.knowledge_base.lock().await = Some(kb);
    Ok("Connected to Qdrant".into())
}

#[tauri::command]
pub async fn search_knowledge(
    query: String,
    collection: Option<String>,
    limit: Option<u64>,
    path_filter: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<tc_core::qdrant::SearchResult>, String> {
    let kb_guard = state.knowledge_base.lock().await;
    let kb = kb_guard.as_ref().ok_or("Qdrant not connected")?;
    let lim = limit.unwrap_or(5);

    // If no collection specified, search across all collections (use largest first)
    if let Some(ref coll) = collection {
        return kb.search(&query, coll, lim, path_filter.as_deref()).await;
    }

    // Auto-discover: search all collections, merge results
    let collections = kb.list_collections().await.unwrap_or_default();
    let mut all_results = Vec::new();
    for coll_info in &collections {
        if let Ok(mut results) = kb.search(&query, &coll_info.name, lim, path_filter.as_deref()).await {
            all_results.append(&mut results);
        }
    }
    all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    all_results.truncate(lim as usize);
    Ok(all_results)
}

#[tauri::command]
pub async fn index_project_knowledge(
    collection: Option<String>,
    subdirectory: Option<String>,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let kb_guard = state.knowledge_base.lock().await;
    let kb = kb_guard.as_ref().ok_or("Qdrant not connected")?;
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or("No project open")?;
    let coll = collection.as_deref().unwrap_or("tauri_code");
    let root = project.root.clone();

    if let Some(ref subdir) = subdirectory {
        let target = root.join(subdir);
        if !target.exists() || !target.is_dir() {
            return Err(format!("Directory not found: {subdir}"));
        }
        kb.index_directory(&root, &target, coll).await
    } else {
        kb.index_project(&root, coll).await
    }
}

#[tauri::command]
pub async fn list_knowledge_collections(
    state: State<'_, AppState>,
) -> Result<Vec<tc_core::qdrant::CollectionInfo>, String> {
    let kb_guard = state.knowledge_base.lock().await;
    let kb = kb_guard.as_ref().ok_or("Qdrant not connected")?;
    kb.list_collections().await
}

/// Browse collection data without requiring embeddings
#[tauri::command]
pub async fn browse_knowledge(
    collection: String,
    limit: Option<u64>,
    state: State<'_, AppState>,
) -> Result<Vec<tc_core::qdrant::SearchResult>, String> {
    let kb_guard = state.knowledge_base.lock().await;
    let kb = kb_guard.as_ref().ok_or("Qdrant not connected")?;
    let lim = limit.unwrap_or(10);
    kb.browse(&collection, lim).await
}

// ──────────────────────────────────────
// Codex Symbiosis commands
// ──────────────────────────────────────

#[tauri::command]
pub async fn deliberate(
    message: String,
    state: State<'_, AppState>,
) -> Result<tc_core::snowglobe::DeliberationResult, String> {
    let router_guard = state.router.lock().await;
    let router = router_guard.as_ref().ok_or("No project open")?;
    let messages = vec![Message { role: "user".into(), content: message }];
    let config = InferenceConfig::default();
    Ok(tc_core::snowglobe::shake(router, &messages, &config).await)
}

#[tauri::command]
pub async fn get_ghost_stats(
    state: State<'_, AppState>,
) -> Result<tc_core::ghost::GhostStats, String> {
    let ghost_guard = state.ghost.lock().await;
    let ghost = ghost_guard.as_ref().ok_or("No project open")?;
    Ok(ghost.stats())
}

// ──────────────────────────────────────
// File operations
// ──────────────────────────────────────

/// Write content to a file within the project (creates parent dirs as needed)
#[tauri::command]
pub async fn write_file_content(
    path: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or("No project open")?;
    let file_path = PathBuf::from(&path);

    // Resolve: if relative, join with project root; if absolute, verify within root
    let resolved = if file_path.is_absolute() {
        file_path
    } else {
        project.root.join(&file_path)
    };

    // Safety: ensure path is within project root
    let canonical_root = project.root.canonicalize().map_err(|e| format!("Root error: {e}"))?;
    // Create parent dirs first so canonicalize works on new files
    if let Some(parent) = resolved.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create dirs: {e}"))?;
    }
    let canonical = resolved.canonicalize()
        .unwrap_or_else(|_| resolved.clone());

    if !canonical.starts_with(&canonical_root) {
        return Err("Cannot write outside project directory".into());
    }

    std::fs::write(&resolved, &content)
        .map_err(|e| format!("Failed to write {}: {e}", path))
}

/// Create a directory within the project
#[tauri::command]
pub async fn create_directory(
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or("No project open")?;
    let dir_path = PathBuf::from(&path);

    let resolved = if dir_path.is_absolute() {
        dir_path
    } else {
        project.root.join(&dir_path)
    };

    let canonical_root = project.root.canonicalize().map_err(|e| format!("Root error: {e}"))?;
    std::fs::create_dir_all(&resolved).map_err(|e| format!("Failed to create directory: {e}"))?;
    let canonical = resolved.canonicalize().map_err(|e| format!("Path error: {e}"))?;

    if !canonical.starts_with(&canonical_root) {
        // Undo if outside root
        std::fs::remove_dir(&resolved).ok();
        return Err("Cannot create directory outside project".into());
    }

    Ok(())
}

/// Delete a file within the project
#[tauri::command]
pub async fn delete_file(
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or("No project open")?;
    let file_path = PathBuf::from(&path);

    let resolved = if file_path.is_absolute() {
        file_path
    } else {
        project.root.join(&file_path)
    };

    let canonical_root = project.root.canonicalize().map_err(|e| format!("Root error: {e}"))?;
    let canonical = resolved.canonicalize().map_err(|e| format!("File not found: {e}"))?;

    if !canonical.starts_with(&canonical_root) {
        return Err("Cannot delete outside project directory".into());
    }

    std::fs::remove_file(&canonical)
        .map_err(|e| format!("Failed to delete {}: {e}", path))
}

/// List all files in the project (full recursive scan, not just key files)
#[tauri::command]
pub async fn list_all_files(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or("No project open")?;

    let mut files: Vec<String> = Vec::new();
    collect_files(&project.root, &project.root, 8, &mut files);
    files.sort();
    Ok(files)
}

/// Compress older conversation messages into a compact digest
/// Preserves key information while reducing token count for local models
fn compress_history(messages: &[Message]) -> String {
    let mut digest = String::new();
    for pair in messages.chunks(2) {
        if pair.is_empty() { continue; }
        let user_msg = &pair[0];
        // Take first 150 chars of user message
        let user_preview: String = user_msg.content.chars().take(150).collect();
        let user_trimmed = user_preview.trim();

        if pair.len() > 1 {
            let ai_msg = &pair[1];
            // Take first 200 chars of AI response
            let ai_preview: String = ai_msg.content.chars().take(200).collect();
            let ai_trimmed = ai_preview.trim();
            digest.push_str(&format!("- User: {} | AI: {}\n", user_trimmed, ai_trimmed));
        } else {
            digest.push_str(&format!("- {}: {}\n", user_msg.role, user_trimmed));
        }
    }
    digest
}

fn collect_files(root: &std::path::Path, dir: &std::path::Path, depth: usize, files: &mut Vec<String>) {
    if depth == 0 { return; }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" || name == "target" || name == "dist" {
            continue;
        }
        if path.is_file() {
            if let Ok(rel) = path.strip_prefix(root) {
                files.push(rel.to_string_lossy().to_string());
            }
        } else if path.is_dir() {
            collect_files(root, &path, depth - 1, files);
        }
    }
}

// ──────────────────────────────────────
// Git operations
// ──────────────────────────────────────

#[tauri::command]
pub async fn git_status(state: State<'_, AppState>) -> Result<tc_core::git::GitStatus, String> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or("No project open")?;
    tc_core::git::status(&project.root)
}

#[tauri::command]
pub async fn git_log(count: Option<u32>, state: State<'_, AppState>) -> Result<Vec<tc_core::git::GitLogEntry>, String> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or("No project open")?;
    tc_core::git::log(&project.root, count.unwrap_or(20))
}

#[tauri::command]
pub async fn git_diff(staged: Option<bool>, state: State<'_, AppState>) -> Result<tc_core::git::GitDiff, String> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or("No project open")?;
    tc_core::git::diff(&project.root, staged.unwrap_or(false))
}

#[tauri::command]
pub async fn git_commit(message: String, state: State<'_, AppState>) -> Result<String, String> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or("No project open")?;
    tc_core::git::commit(&project.root, &message)
}

#[tauri::command]
pub async fn git_push(state: State<'_, AppState>) -> Result<String, String> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or("No project open")?;
    tc_core::git::push(&project.root)
}

#[tauri::command]
pub async fn git_pull(state: State<'_, AppState>) -> Result<String, String> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or("No project open")?;
    tc_core::git::pull(&project.root)
}

// ──────────────────────────────────────
// AI Agent Actions
// ──────────────────────────────────────

#[tauri::command]
pub async fn execute_action(action: tc_core::actions::ProposedAction, state: State<'_, AppState>) -> Result<tc_core::actions::ActionResult, String> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or("No project open")?;
    let root = &project.root;

    match action.details {
        tc_core::actions::ActionDetails::FileWrite { ref path, ref content } => {
            let file_path = std::path::PathBuf::from(path);
            let resolved = if file_path.is_absolute() { file_path } else { root.join(&file_path) };
            if let Some(parent) = resolved.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("Mkdir: {e}"))?;
            }
            std::fs::write(&resolved, content).map_err(|e| format!("Write: {e}"))?;
            Ok(tc_core::actions::ActionResult {
                id: action.id,
                success: true,
                message: format!("Written: {path}"),
            })
        }
        tc_core::actions::ActionDetails::FileDelete { ref path } => {
            let file_path = std::path::PathBuf::from(path);
            let resolved = if file_path.is_absolute() { file_path } else { root.join(&file_path) };
            std::fs::remove_file(&resolved).map_err(|e| format!("Delete: {e}"))?;
            Ok(tc_core::actions::ActionResult {
                id: action.id,
                success: true,
                message: format!("Deleted: {path}"),
            })
        }
        tc_core::actions::ActionDetails::GitCommit { ref message } => {
            let result = tc_core::git::commit(root, message)?;
            Ok(tc_core::actions::ActionResult {
                id: action.id,
                success: true,
                message: result,
            })
        }
        tc_core::actions::ActionDetails::GitPush => {
            let result = tc_core::git::push(root)?;
            Ok(tc_core::actions::ActionResult {
                id: action.id,
                success: true,
                message: result.is_empty().then(|| "Pushed".to_string()).unwrap_or(result),
            })
        }
    }
}
