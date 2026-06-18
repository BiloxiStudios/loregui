mod commands;

use commands::AppState;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let initial_dir = std::env::current_dir().unwrap_or_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            working_dir: Mutex::new(initial_dir),
        })
        .invoke_handler(tauri::generate_handler![
            // Core commands
            commands::open_repository,
            commands::current_repository,
            commands::status,
            commands::log,
            commands::branches,
            commands::stage,
            commands::unstage,
            commands::commit,
            commands::create_branch,
            commands::switch_branch,
            commands::merge_branch,
            commands::push,
            commands::sync,
            commands::create_repository,
            commands::clone,
            // Repository domain (21 ops)
            commands::repo_info,
            commands::repo_dump,
            commands::repo_create,
            commands::repo_create_with_metadata,
            commands::repo_delete,
            commands::repo_release,
            commands::repo_flush,
            commands::repo_gc,
            commands::repo_list,
            commands::repo_verify_state,
            commands::repo_verify_fragment,
            commands::repo_store_immutable_query,
            commands::repo_metadata_get,
            commands::repo_metadata_set,
            commands::repo_metadata_clear,
            commands::repo_instance_list,
            commands::repo_instance_prune,
            commands::repo_update_path,
            commands::repo_config_get,
        ])
        .run(tauri::generate_context!())
        .expect("error while running loregui")
}
