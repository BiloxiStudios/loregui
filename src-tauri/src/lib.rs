mod commands;
mod context;
mod desktop;
mod lan_discovery;
mod operations;
mod server_host;
mod settings;
mod tray;

// Tauri `MockRuntime` IPC harness (test-only). Lives in its own file and is
// compiled only under `cargo test`, so it adds nothing to the shipped binary.
// It exercises the same `#[tauri::command]` set `run()` registers — see the
// module docs and `frontend/e2e/` for the heavier WebDriver smoke suite.
#[cfg(test)]
mod ipc_harness_tests;

use commands::AppState;
use context::{
    context_get, context_select, context_update, context_validate, ContextSelectionCoordinator,
};
use desktop::{get_desktop_settings, set_autostart, set_close_to_tray};
use operations::subscribe::subscribe_notifications;
use operations::unsubscribe::unsubscribe_notifications;
use settings::SettingsManager;
use std::collections::HashSet;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        // In-app auto-update (SBAI-4040). The frontend calls `check()` →
        // `downloadAndInstall()` → `relaunch()`; the process plugin provides
        // relaunch() after the updater swaps the running binary.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .setup(|app| {
            // Load persisted desktop settings (autostart, close-to-tray) from the
            // app config directory.
            let config_dir = app.path().app_config_dir().unwrap_or_else(|_| {
                tracing::warn!("could not resolve app config dir, using fallback");
                std::env::temp_dir().join("loregui")
            });
            app.manage(SettingsManager::new(config_dir));

            // Treat the remembered local path as untrusted input. It becomes
            // active only after the real repository backend validates it; stale
            // or corrupt state is cleared and startup remains repository-free.
            let state = app.state::<AppState>();
            let settings = app.state::<SettingsManager>();
            tauri::async_runtime::block_on(commands::restore_active_repository(&state, &settings));

            // Install the single system tray (status icon + quick actions).
            tray::install(app.handle())?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let settings = window.state::<SettingsManager>();
                if settings.get().close_to_tray {
                    // Hide to tray instead of quitting.
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    // The window is actually closing (app will quit). Flush any
                    // in-flight lore store writes for the open repo before we go
                    // — a long-lived session's background flush may not have
                    // landed yet (see commands::flush_working_tree). Best-effort.
                    let state = window.state::<AppState>();
                    tauri::async_runtime::block_on(commands::flush_app_state(&state));
                }
            }
        })
        .manage(AppState {
            working_dir: Mutex::new(None),
            context_selection: Mutex::new(ContextSelectionCoordinator::default()),
            subscription_counter: AtomicU64::new(0),
            subscriptions: Mutex::new(HashSet::new()),
            storage_session: Mutex::new(commands::StorageSession::default()),
            hosted_server: Mutex::new(None),
            advertised_url: Mutex::new(None),
            lock_inbox: Mutex::new(Vec::new()),
            lock_request_counter: AtomicU64::new(0),
            lan_announcer: Mutex::new(None),
            lan_browser: Mutex::new(None),
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
            commands::file_dump,
            commands::file_stage,
            commands::file_dirty,
            commands::file_dirty_copy,
            commands::file_dirty_move,
            commands::file_obliterate,
            commands::file_reset_to_last_merged,
            commands::file_diff,
            commands::repository_dump,
            commands::repository_delete,
            commands::repository_list,
            commands::repository_instance_list,
            commands::repository_instance_prune,
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
            commands::lock_file_acquire,
            commands::lock_file_status,
            commands::lock_request_checkin,
            commands::lock_inbox_list,
            commands::lock_inbox_dismiss,
            commands::branch_reset,
            commands::branch_merge_start,
            commands::branch_merge_restart,
            commands::branch_merge_resolve_theirs,
            commands::branch_merge_resolve_mine,
            commands::branch_merge_resolve,
            commands::branch_latest_list,
            commands::branch_list,
            commands::branch_create,
            commands::layer_add,
            commands::repository_create,
            commands::dependency_add,
            commands::dependency_list,
            commands::dependency_remove,
            commands::link_add,
            commands::link_remove,
            commands::storage_open,
            commands::storage_put,
            commands::storage_get,
            commands::storage_obliterate,
            commands::storage_open_handle,
            commands::storage_close,
            commands::storage_flush,
            commands::storage_get_metadata,
            commands::storage_put_file,
            commands::storage_copy,
            commands::storage_upload,
            commands::storage_mutable_store,
            commands::storage_mutable_load,
            commands::storage_mutable_list,
            commands::storage_mutable_compare_and_swap,
            commands::shared_store_create,
            commands::host_store_prepare,
            commands::host_store_probe,
            commands::shared_store_info,
            commands::shared_store_set_use_automatically,
            commands::repository_clone,
            commands::auth_login_interactive,
            commands::auth_login_with_token,
            commands::auth_user_info,
            commands::auth_logout,
            commands::auth_clear,
            commands::revision_cherry_pick_restart,
            commands::service_start,
            commands::service_stop,
            commands::host_server_start,
            commands::host_server_render_config,
            commands::host_server_stop,
            commands::host_server_restart,
            commands::host_server_status,
            commands::host_server_set_advertised_url,
            commands::host_server_clear_advertised_url,
            commands::repository_info,
            commands::repository_release,
            commands::repository_urc_status,
            commands::repository_recover_local,
            commands::repository_config_get,
            commands::repository_metadata_clear,
            commands::repository_create_with_metadata,
            commands::repository_store_immutable_query,
            commands::repository_verify_fragment,
            commands::repository_update_path,
            commands::file_hash,
            commands::file_metadata_list,
            commands::revision_revert_abort,
            commands::revision_revert_resolve_mine,
            commands::revision_commit_with_metadata,
            commands::revision_metadata_clear,
            commands::revision_activity_report,
            commands::read_license_file,
            commands::read_text_file,
            commands::read_file_bytes,
            commands::write_text_file,
            commands::tray_sync_state,
            commands::lan_discover_browse,
            commands::lan_discover_refresh,
            commands::lan_discover_stop,
            get_desktop_settings,
            set_autostart,
            set_close_to_tray,
            subscribe_notifications,
            unsubscribe_notifications,
            context_get,
            context_select,
            context_update,
            context_validate,
        ])
        .build(tauri::generate_context!())
        .expect("error while building loregui")
        .run(|app_handle, event| {
            // App-exit backstop (SBAI server-host hardening). On any
            // exit-requested / exit event, stop the hosted `loreserver` child if
            // one is still running so it does not outlive the GUI and keep
            // holding its ports (41337 / 41339). `HostedServer`'s `Drop` is the
            // other half of this belt-and-suspenders: this hook handles the clean
            // quit path, Drop handles slot-overwrite / teardown paths.
            //
            // We also flush the open repo's deferred store writes here so a quit
            // that does not route through a window CloseRequested (e.g. tray
            // "Quit" / `app.exit()`) is still durable.
            if let tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit = event {
                let state = app_handle.state::<AppState>();
                tauri::async_runtime::block_on(commands::flush_app_state(&state));
                let mut slot = state
                    .hosted_server
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let _ = server_host::stop(&mut slot);
            }
        });
}
