use crate::state::{AppConfig, AppState};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tc_core::context::ProjectContext;
use tc_core::ghost::GhostEntryKind;
use tc_core::models::*;
use tc_core::prompt::CoreSource;
use tc_core::router::{RouteDecision, TemperatureMode};

type ApiResult<T> = Result<Json<T>, (StatusCode, String)>;

fn err(msg: String) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, msg)
}

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/open_project", post(open_project))
        .route("/chat", post(chat))
        .route("/list_models", post(list_models))
        .route("/ghost_remember", post(ghost_remember))
        .route("/get_file_tree", post(get_file_tree))
        .route("/read_file", post(read_file))
        .route("/save_settings", post(save_settings))
        .route("/load_settings", post(load_settings))
        .route("/update_system_prompt", post(update_system_prompt))
        .route("/evolve_core_prompt", post(evolve_core_prompt))
        .route("/set_prompt_ratio", post(set_prompt_ratio))
        .route("/get_prompt_state", post(get_prompt_state))
        .route("/search_knowledge", post(search_knowledge))
        .route("/index_project_knowledge", post(index_project_knowledge))
        .route("/list_knowledge_collections", post(list_knowledge_collections))
        .route("/browse_knowledge", post(browse_knowledge))
        .route("/connect_qdrant", post(connect_qdrant))
        .route("/deliberate", post(deliberate))
        .route("/get_ghost_stats", post(get_ghost_stats))
        .route("/write_file_content", post(write_file_content))
        .route("/create_directory", post(create_directory))
        .route("/delete_file", post(delete_file_handler))
        .route("/list_all_files", post(list_all_files))
        .route("/git_status", post(git_status))
        .route("/git_log", post(git_log))
        .route("/git_diff", post(git_diff))
        .route("/git_commit", post(git_commit))
        .route("/git_push", post(git_push))
        .route("/git_pull", post(git_pull))
        .route("/execute_action", post(execute_action))
        .route("/clear_conversation", post(clear_conversation))
}

// ──────────────────────────────────────
// Request/Response types
// ──────────────────────────────────────

#[derive(Deserialize)]
struct OpenProjectReq {
    path: String,
}

#[derive(Deserialize)]
struct ChatReq {
    message: String,
    model_override: Option<String>,
    temperature: Option<f32>,
}

#[derive(Serialize)]
struct ChatResp {
    content: String,
    decision: RouteDecision,
    guardian: Option<tc_core::quinn::GuardianReport>,
}

#[derive(Deserialize)]
struct GhostRememberReq {
    kind: String,
    content: String,
}

#[derive(Deserialize)]
struct ReadFileReq {
    path: String,
}

#[derive(Deserialize)]
struct UpdatePromptReq {
    prompt: String,
}

#[derive(Deserialize)]
struct EvolvePromptReq {
    insight: String,
    source: String,
}

#[derive(Deserialize)]
struct SetRatioReq {
    ratio: f32,
}

#[derive(Serialize)]
struct PromptStateResp {
    system_prompt: String,
    core_entries_count: usize,
    core_preview: String,
    ratio: f32,
}

#[derive(Deserialize)]
struct SearchReq {
    query: String,
    collection: Option<String>,
    limit: Option<u64>,
    path_filter: Option<String>,
}

#[derive(Deserialize)]
struct BrowseReq {
    collection: String,
    limit: Option<u64>,
}

#[derive(Deserialize)]
struct IndexReq {
    collection: Option<String>,
    subdirectory: Option<String>,
}

#[derive(Deserialize)]
struct DeliberateReq {
    message: String,
}

#[derive(Deserialize)]
struct WriteFileReq {
    path: String,
    content: String,
}

#[derive(Deserialize)]
struct CreateDirReq {
    path: String,
}

#[derive(Deserialize)]
struct DeleteFileReq {
    path: String,
}

// ──────────────────────────────────────
// Route handlers
// ──────────────────────────────────────

async fn open_project(
    State(state): State<AppState>,
    Json(req): Json<OpenProjectReq>,
) -> ApiResult<ProjectContext> {
    // In web mode, if no path is provided, fall back to DEFAULT_PROJECT_PATH env var
    let raw_path = if req.path.is_empty() {
        std::env::var("DEFAULT_PROJECT_PATH").unwrap_or_default()
    } else {
        req.path
    };
    if raw_path.is_empty() {
        return Err(err("No project path provided".into()));
    }
    let project_path = PathBuf::from(&raw_path);

    let ctx = ProjectContext::scan(&project_path);
    let ghost = tc_core::ghost::Ghost::load_or_create(&project_path);
    let prompt_engine = tc_core::prompt::PromptEngine::load_or_create(&project_path);

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
        models.push(Arc::new(cloud::ClaudeModel::sonnet(api_key)));
    }

    let router = tc_core::router::Router::new_with_config(models, config.to_routing_config());

    *state.router.lock().await = Some(router);
    *state.ghost.lock().await = Some(ghost);
    *state.prompt_engine.lock().await = Some(prompt_engine);
    *state.project.lock().await = Some(ctx.clone());

    // Clear conversation history on new project
    state.conversation.lock().await.clear();

    // Auto-connect Qdrant and ensure per-project collection
    let qdrant_url = config.qdrant_url.as_deref().unwrap_or("http://localhost:6333").to_string();
    let embedding_port = config.lm_studio_port;
    drop(config);
    if let Ok(kb) = tc_core::qdrant::KnowledgeBase::connect(&qdrant_url, embedding_port).await {
        let collection_name = format!("project-{}", ctx.name.to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
            .collect::<String>());
        let _ = kb.ensure_project_collection(&collection_name).await;
        *state.knowledge_base.lock().await = Some(kb);
    }

    Ok(Json(ctx))
}

async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatReq>,
) -> ApiResult<ChatResp> {
    let router_guard = state.router.lock().await;
    let router = router_guard.as_ref().ok_or(err("No project open".into()))?;

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
                    if let Ok(results) = kb.search(&req.message, &coll_info.name, 3, None).await {
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
        messages.push(Message { role: "system".into(), content: system_ctx });
    }

    // Include conversation history with smart compression for local models
    {
        let history = state.conversation.lock().await;
        if history.len() > 10 {
            let older = &history[..history.len() - 10];
            let recent = &history[history.len() - 10..];

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

    let user_message = req.message.clone();

    // Pre-route complexity check for local model nudges
    let complexity = tc_core::router::Router::score_complexity(&messages);

    // CoT nudge: if task is non-trivial and will likely go to local model → prefix with step-by-step
    let final_message = if complexity > 0.4 && complexity < router.config.cloud_floor {
        format!("Premysli krok za krokom:\n\n{}", req.message)
    } else {
        req.message
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

    messages.push(Message { role: "user".into(), content: final_message });

    let config = InferenceConfig {
        temperature: req.temperature.unwrap_or(0.7),
        ..Default::default()
    };

    let (decision, content) = if let Some(ref model_name) = req.model_override {
        let content = router
            .route_to(model_name, &messages, &config)
            .await
            .map_err(|e| err(e.to_string()))?;
        let decision = RouteDecision {
            model_name: model_name.clone(),
            tier: ModelTier::Local,
            complexity: tc_core::router::Router::score_complexity(&messages),
            reason: "User override".into(),
            temperature_mode: TemperatureMode::default(),
        };
        (decision, content)
    } else {
        router.route(&messages, &config).await.map_err(|e| err(e.to_string()))?
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

    // Barycentrum observation
    {
        let mut ghost_write = state.ghost.lock().await;
        if let Some(ref mut ghost) = *ghost_write {
            tc_core::barycentrum::observe_and_record(ghost, &user_message, &content);
        }
    }

    let guardian = if guardian_report.needs_attention { Some(guardian_report) } else { None };
    Ok(Json(ChatResp { content, decision, guardian }))
}

async fn list_models(
    State(state): State<AppState>,
) -> ApiResult<Vec<ModelInfo>> {
    let config = state.config.lock().await;
    let mut models = local::LmStudioModel::discover(config.lm_studio_port).await;

    if config.claude_api_key.is_some() {
        models.push(ModelInfo {
            id: "claude-sonnet-4-5".into(),
            name: "Claude Sonnet 4.5".into(),
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

    Ok(Json(models))
}

async fn ghost_remember(
    State(state): State<AppState>,
    Json(req): Json<GhostRememberReq>,
) -> ApiResult<()> {
    let mut ghost_guard = state.ghost.lock().await;
    let ghost = ghost_guard.as_mut().ok_or(err("No project open".into()))?;

    let entry_kind = match req.kind.as_str() {
        "decision" => GhostEntryKind::Decision,
        "success" => GhostEntryKind::Success,
        "failure" => GhostEntryKind::Failure,
        "preference" => GhostEntryKind::Preference,
        "emergence" => GhostEntryKind::Emergence,
        _ => GhostEntryKind::ProjectContext,
    };

    ghost.remember(entry_kind, req.content);
    Ok(Json(()))
}

async fn get_file_tree(
    State(state): State<AppState>,
) -> ApiResult<Vec<String>> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or(err("No project open".into()))?;
    Ok(Json(project.key_files.iter().map(|f| f.path.clone()).collect()))
}

async fn read_file(
    State(_state): State<AppState>,
    Json(req): Json<ReadFileReq>,
) -> ApiResult<String> {
    let content = std::fs::read_to_string(&req.path)
        .map_err(|e| err(format!("Failed to read {}: {e}", req.path)))?;
    Ok(Json(content))
}

async fn save_settings(
    State(state): State<AppState>,
    Json(settings): Json<AppConfig>,
) -> ApiResult<()> {
    settings.save();
    *state.config.lock().await = settings;
    Ok(Json(()))
}

async fn load_settings(
    State(state): State<AppState>,
) -> ApiResult<AppConfig> {
    let config = state.config.lock().await;
    Ok(Json(config.clone()))
}

async fn update_system_prompt(
    State(state): State<AppState>,
    Json(req): Json<UpdatePromptReq>,
) -> ApiResult<()> {
    let mut engine_guard = state.prompt_engine.lock().await;
    let engine = engine_guard.as_mut().ok_or(err("No project open".into()))?;
    engine.set_system_prompt(req.prompt);
    Ok(Json(()))
}

async fn evolve_core_prompt(
    State(state): State<AppState>,
    Json(req): Json<EvolvePromptReq>,
) -> ApiResult<()> {
    let mut engine_guard = state.prompt_engine.lock().await;
    let engine = engine_guard.as_mut().ok_or(err("No project open".into()))?;

    let core_source = match req.source.as_str() {
        "observation" => CoreSource::Observation,
        "user" => CoreSource::UserGuidance,
        "emergence" => CoreSource::Emergence,
        "pattern" => CoreSource::ProjectPattern,
        _ => CoreSource::Observation,
    };

    engine.evolve(req.insight, core_source);
    Ok(Json(()))
}

async fn set_prompt_ratio(
    State(state): State<AppState>,
    Json(req): Json<SetRatioReq>,
) -> ApiResult<()> {
    let mut engine_guard = state.prompt_engine.lock().await;
    let engine = engine_guard.as_mut().ok_or(err("No project open".into()))?;
    engine.set_ratio(req.ratio);
    Ok(Json(()))
}

async fn get_prompt_state(
    State(state): State<AppState>,
) -> ApiResult<PromptStateResp> {
    let engine_guard = state.prompt_engine.lock().await;
    let engine = engine_guard.as_ref().ok_or(err("No project open".into()))?;

    let core_preview = engine
        .core_entries
        .iter()
        .rev()
        .take(5)
        .map(|e| e.insight.clone())
        .collect::<Vec<_>>()
        .join("\n");

    Ok(Json(PromptStateResp {
        system_prompt: engine.system_prompt.clone(),
        core_entries_count: engine.core_entries.len(),
        core_preview,
        ratio: engine.ratio,
    }))
}

async fn connect_qdrant(
    State(state): State<AppState>,
) -> ApiResult<String> {
    let config = state.config.lock().await;
    let url = config.qdrant_url.as_deref().unwrap_or("http://localhost:6333");
    let port = config.lm_studio_port;

    let kb = tc_core::qdrant::KnowledgeBase::connect(url, port)
        .await
        .map_err(|e| err(e))?;
    *state.knowledge_base.lock().await = Some(kb);
    Ok(Json("Connected to Qdrant".into()))
}

async fn search_knowledge(
    State(state): State<AppState>,
    Json(req): Json<SearchReq>,
) -> ApiResult<Vec<tc_core::qdrant::SearchResult>> {
    let kb_guard = state.knowledge_base.lock().await;
    let kb = kb_guard.as_ref().ok_or(err("Qdrant not connected".into()))?;
    let coll = req.collection.as_deref().unwrap_or("tauri_code");
    let lim = req.limit.unwrap_or(5);
    let results = kb.search(&req.query, coll, lim, req.path_filter.as_deref())
        .await
        .map_err(|e| err(e))?;
    Ok(Json(results))
}

async fn index_project_knowledge(
    State(state): State<AppState>,
    Json(req): Json<IndexReq>,
) -> ApiResult<usize> {
    let kb_guard = state.knowledge_base.lock().await;
    let kb = kb_guard.as_ref().ok_or(err("Qdrant not connected".into()))?;
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or(err("No project open".into()))?;
    let coll = req.collection.as_deref().unwrap_or("tauri_code");
    let root = project.root.clone();
    let count = if let Some(ref subdir) = req.subdirectory {
        let target = root.join(subdir);
        if !target.exists() || !target.is_dir() {
            return Err(err(format!("Directory not found: {subdir}")));
        }
        kb.index_directory(&root, &target, coll).await.map_err(|e| err(e))?
    } else {
        kb.index_project(&root, coll).await.map_err(|e| err(e))?
    };
    Ok(Json(count))
}

async fn list_knowledge_collections(
    State(state): State<AppState>,
) -> ApiResult<Vec<tc_core::qdrant::CollectionInfo>> {
    let kb_guard = state.knowledge_base.lock().await;
    let kb = kb_guard.as_ref().ok_or(err("Qdrant not connected".into()))?;
    let collections = kb.list_collections().await.map_err(|e| err(e))?;
    Ok(Json(collections))
}

async fn browse_knowledge(
    State(state): State<AppState>,
    Json(req): Json<BrowseReq>,
) -> ApiResult<Vec<tc_core::qdrant::SearchResult>> {
    let kb_guard = state.knowledge_base.lock().await;
    let kb = kb_guard.as_ref().ok_or(err("Qdrant not connected".into()))?;
    let results = kb.browse(&req.collection, req.limit.unwrap_or(10))
        .await
        .map_err(|e| err(e))?;
    Ok(Json(results))
}

async fn deliberate(
    State(state): State<AppState>,
    Json(req): Json<DeliberateReq>,
) -> ApiResult<tc_core::snowglobe::DeliberationResult> {
    let router_guard = state.router.lock().await;
    let router = router_guard.as_ref().ok_or(err("No project open".into()))?;
    let messages = vec![Message { role: "user".into(), content: req.message }];
    let config = InferenceConfig::default();
    Ok(Json(tc_core::snowglobe::shake(router, &messages, &config).await))
}

async fn get_ghost_stats(
    State(state): State<AppState>,
) -> ApiResult<tc_core::ghost::GhostStats> {
    let ghost_guard = state.ghost.lock().await;
    let ghost = ghost_guard.as_ref().ok_or(err("No project open".into()))?;
    Ok(Json(ghost.stats()))
}

// ──────────────────────────────────────
// File operations
// ──────────────────────────────────────

async fn write_file_content(
    State(state): State<AppState>,
    Json(req): Json<WriteFileReq>,
) -> ApiResult<()> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or(err("No project open".into()))?;
    let file_path = PathBuf::from(&req.path);
    let resolved = if file_path.is_absolute() { file_path } else { project.root.join(&file_path) };
    let canonical_root = project.root.canonicalize().map_err(|e| err(format!("Root error: {e}")))?;
    if let Some(parent) = resolved.parent() {
        std::fs::create_dir_all(parent).map_err(|e| err(format!("Mkdir error: {e}")))?;
    }
    let canonical = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());
    if !canonical.starts_with(&canonical_root) {
        return Err(err("Cannot write outside project".into()));
    }
    std::fs::write(&resolved, &req.content).map_err(|e| err(format!("Write error: {e}")))?;
    Ok(Json(()))
}

async fn create_directory(
    State(state): State<AppState>,
    Json(req): Json<CreateDirReq>,
) -> ApiResult<()> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or(err("No project open".into()))?;
    let dir_path = PathBuf::from(&req.path);
    let resolved = if dir_path.is_absolute() { dir_path } else { project.root.join(&dir_path) };
    let canonical_root = project.root.canonicalize().map_err(|e| err(format!("Root error: {e}")))?;
    std::fs::create_dir_all(&resolved).map_err(|e| err(format!("Mkdir error: {e}")))?;
    let canonical = resolved.canonicalize().map_err(|e| err(format!("Path error: {e}")))?;
    if !canonical.starts_with(&canonical_root) {
        std::fs::remove_dir(&resolved).ok();
        return Err(err("Cannot create directory outside project".into()));
    }
    Ok(Json(()))
}

async fn delete_file_handler(
    State(state): State<AppState>,
    Json(req): Json<DeleteFileReq>,
) -> ApiResult<()> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or(err("No project open".into()))?;
    let file_path = PathBuf::from(&req.path);
    let resolved = if file_path.is_absolute() { file_path } else { project.root.join(&file_path) };
    let canonical_root = project.root.canonicalize().map_err(|e| err(format!("Root error: {e}")))?;
    let canonical = resolved.canonicalize().map_err(|e| err(format!("File not found: {e}")))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(err("Cannot delete outside project".into()));
    }
    std::fs::remove_file(&canonical).map_err(|e| err(format!("Delete error: {e}")))?;
    Ok(Json(()))
}

async fn list_all_files(
    State(state): State<AppState>,
) -> ApiResult<Vec<String>> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or(err("No project open".into()))?;
    let mut files: Vec<String> = Vec::new();
    collect_files(&project.root, &project.root, 8, &mut files);
    files.sort();
    Ok(Json(files))
}

// ──────────────────────────────────────
// Git operations
// ──────────────────────────────────────

#[derive(Deserialize)]
struct GitLogReq {
    count: Option<u32>,
}

#[derive(Deserialize)]
struct GitDiffReq {
    staged: Option<bool>,
}

#[derive(Deserialize)]
struct GitCommitReq {
    message: String,
}

async fn git_status(
    State(state): State<AppState>,
) -> ApiResult<tc_core::git::GitStatus> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or(err("No project open".into()))?;
    tc_core::git::status(&project.root).map(Json).map_err(|e| err(e))
}

async fn git_log(
    State(state): State<AppState>,
    Json(req): Json<GitLogReq>,
) -> ApiResult<Vec<tc_core::git::GitLogEntry>> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or(err("No project open".into()))?;
    tc_core::git::log(&project.root, req.count.unwrap_or(10)).map(Json).map_err(|e| err(e))
}

async fn git_diff(
    State(state): State<AppState>,
    Json(req): Json<GitDiffReq>,
) -> ApiResult<tc_core::git::GitDiff> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or(err("No project open".into()))?;
    tc_core::git::diff(&project.root, req.staged.unwrap_or(false)).map(Json).map_err(|e| err(e))
}

async fn git_commit(
    State(state): State<AppState>,
    Json(req): Json<GitCommitReq>,
) -> ApiResult<String> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or(err("No project open".into()))?;
    tc_core::git::commit(&project.root, &req.message).map(Json).map_err(|e| err(e))
}

async fn git_push(
    State(state): State<AppState>,
) -> ApiResult<String> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or(err("No project open".into()))?;
    tc_core::git::push(&project.root).map(Json).map_err(|e| err(e))
}

async fn git_pull(
    State(state): State<AppState>,
) -> ApiResult<String> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or(err("No project open".into()))?;
    tc_core::git::pull(&project.root).map(Json).map_err(|e| err(e))
}

// ──────────────────────────────────────
// AI Agent Actions
// ──────────────────────────────────────

async fn execute_action(
    State(state): State<AppState>,
    Json(action): Json<tc_core::actions::ProposedAction>,
) -> ApiResult<tc_core::actions::ActionResult> {
    let project_guard = state.project.lock().await;
    let project = project_guard.as_ref().ok_or(err("No project open".into()))?;
    let root = &project.root;

    match action.details {
        tc_core::actions::ActionDetails::FileWrite { ref path, ref content } => {
            let file_path = PathBuf::from(path);
            let resolved = if file_path.is_absolute() { file_path } else { root.join(&file_path) };
            if let Some(parent) = resolved.parent() {
                std::fs::create_dir_all(parent).map_err(|e| err(format!("Mkdir: {e}")))?;
            }
            std::fs::write(&resolved, content).map_err(|e| err(format!("Write: {e}")))?;
            Ok(Json(tc_core::actions::ActionResult {
                id: action.id,
                success: true,
                message: format!("Written: {path}"),
            }))
        }
        tc_core::actions::ActionDetails::FileDelete { ref path } => {
            let file_path = PathBuf::from(path);
            let resolved = if file_path.is_absolute() { file_path } else { root.join(&file_path) };
            std::fs::remove_file(&resolved).map_err(|e| err(format!("Delete: {e}")))?;
            Ok(Json(tc_core::actions::ActionResult {
                id: action.id,
                success: true,
                message: format!("Deleted: {path}"),
            }))
        }
        tc_core::actions::ActionDetails::GitCommit { ref message } => {
            let result = tc_core::git::commit(root, message).map_err(|e| err(e))?;
            Ok(Json(tc_core::actions::ActionResult {
                id: action.id,
                success: true,
                message: result,
            }))
        }
        tc_core::actions::ActionDetails::GitPush => {
            let result = tc_core::git::push(root).map_err(|e| err(e))?;
            Ok(Json(tc_core::actions::ActionResult {
                id: action.id,
                success: true,
                message: if result.is_empty() { "Pushed".into() } else { result },
            }))
        }
    }
}

// ──────────────────────────────────────
// Conversation management
// ──────────────────────────────────────

async fn clear_conversation(
    State(state): State<AppState>,
) -> ApiResult<()> {
    state.conversation.lock().await.clear();
    Ok(Json(()))
}

/// Compress older conversation messages into a compact digest
fn compress_history(messages: &[Message]) -> String {
    let mut digest = String::new();
    for pair in messages.chunks(2) {
        if pair.is_empty() { continue; }
        let user_msg = &pair[0];
        let user_preview: String = user_msg.content.chars().take(150).collect();
        let user_trimmed = user_preview.trim();

        if pair.len() > 1 {
            let ai_msg = &pair[1];
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
