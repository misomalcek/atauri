pub mod commands;
pub mod stream;

use commands::AppState;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load persisted config or use defaults
    let config = commands::AppConfig::load();

    let state = AppState {
        router: Arc::new(Mutex::new(None)),
        ghost: Arc::new(Mutex::new(None)),
        project: Arc::new(Mutex::new(None)),
        prompt_engine: Arc::new(Mutex::new(None)),
        knowledge_base: Arc::new(Mutex::new(None)),
        config: Arc::new(Mutex::new(config)),
        conversation: Arc::new(Mutex::new(Vec::new())),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::open_project,
            commands::chat,
            commands::list_models,
            commands::ghost_remember,
            commands::get_file_tree,
            commands::read_file,
            commands::save_settings,
            commands::load_settings,
            commands::update_system_prompt,
            commands::evolve_core_prompt,
            commands::set_prompt_ratio,
            commands::get_prompt_state,
            commands::search_knowledge,
            commands::index_project_knowledge,
            commands::list_knowledge_collections,
            commands::connect_qdrant,
            commands::browse_knowledge,
            stream::chat_stream,
            commands::deliberate,
            commands::get_ghost_stats,
            commands::write_file_content,
            commands::create_directory,
            commands::delete_file,
            commands::list_all_files,
            commands::git_status,
            commands::git_log,
            commands::git_diff,
            commands::git_commit,
            commands::git_push,
            commands::git_pull,
            commands::execute_action,
            commands::clear_conversation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
