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
            storage_session: Mutex::new(commands::StorageSession::default()),
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
            commands::branch_unprotect,
            commands::branch_archive,
            commands::branch_metadata_get,
            commands::branch_merge_abort,
            commands::branch_merge_unresolve,
            commands::branch_merge_into,
            commands::file_info,
            commands::file_write,
            commands::file_stage,
            commands::file_dirty,
            commands::file_dirty_copy,
            commands::file_dirty_move,
            commands::file_obliterate,
            commands::repository_dump,
            commands::repository_delete,
            commands::repository_list,
            commands::repository_instance_list,
            commands::repository_verify_state,
            commands::repository_flush,
            commands::repository_gc,
            commands::repository_metadata_get,
            commands::repository_metadata_set,
            commands::revision_diff,
            commands::revision_find,
            commands::revision_find_local,
            commands::revision_revert_local,
            commands::revision_sync,
            commands::revision_history,
            commands::revision_info,
            commands::revision_amend,
            commands::revision_commit,
            commands::revision_revert_resolve,
            commands::auth_local_user_info,
            commands::lock_file_release,
            commands::lock_file_acquire_as_owner,
            commands::lock_file_query,
            commands::branch_reset,
            commands::branch_merge_start,
            commands::branch_merge_restart,
            commands::branch_merge_resolve_theirs,
            commands::branch_merge_resolve_mine,
            commands::branch_merge_resolve,
            commands::branch_latest_list,
            commands::branch_list,
            commands::branch_create,
            commands::repository_create,
            commands::link_remove,
            commands::storage_open,
            commands::storage_put,
            commands::storage_get,
            commands::storage_obliterate,
            commands::shared_store_create,
            commands::repository_clone,
            commands::auth_login_interactive,
            commands::auth_login_with_token,
            commands::auth_user_info,
            commands::service_start,
            subscribe_notifications,
            unsubscribe_notifications,
        ])
        .run(tauri::generate_context!())
        .expect("error while running loregui");
}
