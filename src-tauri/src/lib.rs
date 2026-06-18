mod commands;
mod operations;

use commands::AppState;
use operations::subscribe::subscribe_notifications;
use operations::unsubscribe::unsubscribe_notifications;
use std::collections::HashSet;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let initial_dir = std::env::current_dir().unwrap_or_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            working_dir: Mutex::new(initial_dir),
            subscription_counter: AtomicU64::new(0),
            subscriptions: Mutex::new(HashSet::new()),
        })
        .invoke_handler(tauri::generate_handler![
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
            commands::branch_info,
            commands::branch_protect,
            commands::branch_archive,
            commands::branch_merge_unresolve,
            commands::file_obliterate,
            commands::revision_diff,
            subscribe_notifications,
            unsubscribe_notifications,
        ])
        .run(tauri::generate_context!())
        .expect("error while running loregui");
}
