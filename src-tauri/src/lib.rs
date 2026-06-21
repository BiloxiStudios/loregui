mod commands;
mod desktop;
mod operations;
mod settings;

use commands::AppState;
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

    let initial_dir = std::env::current_dir().unwrap_or_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .setup(|app| {
            // Load settings from the app config directory.
            let config_dir = app.path().app_config_dir().unwrap_or_else(|_| {
                tracing::warn!("could not resolve app config dir, using fallback");
                std::env::temp_dir().join("loregui")
            });
            app.manage(SettingsManager::new(config_dir));

            // Set up the system tray icon.
            setup_tray(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let settings = window.state::<SettingsManager>();
                if settings.get().close_to_tray {
                    // Prevent the window from closing; hide it to tray instead.
                    api.prevent_close();
                    let _ = window.hide();
                }
                // Otherwise let the close proceed normally (app quits).
            }
        })
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
            commands::shared_store_create,
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
            commands::repository_info,
            commands::repository_release,
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
            subscribe_notifications,
            unsubscribe_notifications,
            get_desktop_settings,
            set_autostart,
            set_close_to_tray,
        ])
        .run(tauri::generate_context!())
        .expect("error while running loregui");
}

/// Set up the system tray icon with a menu for quick actions.
fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::TrayIconBuilder;

    let show_item = MenuItemBuilder::with_id("show", "Open LoreGUI").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
    let menu = MenuBuilder::new(app)
        .items(&[&show_item, &quit_item])
        .build()?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("LoreGUI")
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                // Show and focus the main window.
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
