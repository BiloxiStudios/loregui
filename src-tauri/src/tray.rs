use serde::{Deserialize, Serialize};
use tauri::{
    image::Image,
    menu::{Menu, MenuItemBuilder, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};

pub const TRAY_ACTION_EVENT: &str = "tray/action";
const TRAY_ID: &str = "loregui-tray";
const TRAY_SEPARATOR: &str = " · ";
const STATUS_DOT_OFFSET: i32 = 7;
const STATUS_DOT_OUTER_RADIUS: i32 = 5;
const STATUS_DOT_INNER_RADIUS: i32 = 4;
const COLOR_STATUS_BORDER: [u8; 4] = [255, 255, 255, 255];
const COLOR_STATUS_CLEAN: [u8; 4] = [73, 191, 115, 255];
const COLOR_STATUS_DIRTY: [u8; 4] = [230, 168, 58, 255];
const COLOR_STATUS_SYNCING: [u8; 4] = [84, 160, 255, 255];
const COLOR_STATUS_CONFLICT: [u8; 4] = [230, 92, 92, 255];

const TRAY_OPEN_ID: &str = "tray.open";
const TRAY_SYNC_ID: &str = "tray.sync";
const TRAY_CHECK_IN_ID: &str = "tray.check_in";
const TRAY_RELEASE_LOCK_ID: &str = "tray.release_lock";
const TRAY_CHECK_UPDATES_ID: &str = "tray.check_updates";
const TRAY_QUIT_ID: &str = "tray.quit";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TrayStatusKind {
    #[default]
    Clean,
    Dirty,
    Syncing,
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TraySnapshot {
    pub branch: String,
    pub dirty_count: usize,
    #[serde(default)]
    pub status: TrayStatusKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayActionPayload {
    pub action: String,
}

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let snapshot = TraySnapshot::default();
    let tooltip = format_tooltip(&snapshot);
    let title = format_title(&snapshot);
    let icon = icon_for_status(snapshot.status)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .icon(icon)
        .tooltip(tooltip)
        .title(title)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_OPEN_ID => {
                let _ = show_main_window(app);
            }
            TRAY_SYNC_ID => {
                let _ = emit_action(app, "sync");
            }
            TRAY_CHECK_IN_ID => {
                let _ = show_main_window(app);
                let _ = emit_action(app, "check-in");
            }
            TRAY_RELEASE_LOCK_ID => {
                let _ = show_main_window(app);
                let _ = emit_action(app, "release-lock");
            }
            TRAY_CHECK_UPDATES_ID => {
                let _ = show_main_window(app);
                let _ = emit_action(app, "check-updates");
            }
            TRAY_QUIT_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = toggle_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

pub fn apply_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    snapshot: &TraySnapshot,
) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_icon(Some(icon_for_status(snapshot.status)?))?;
        tray.set_tooltip(Some(format_tooltip(snapshot)))?;
        tray.set_title(Some(format_title(snapshot)))?;
    }
    Ok(())
}

fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    let open = MenuItemBuilder::with_id(TRAY_OPEN_ID, "Open LoreGUI").build(app)?;
    let sync = MenuItemBuilder::with_id(TRAY_SYNC_ID, "Sync").build(app)?;
    let check_in = MenuItemBuilder::with_id(TRAY_CHECK_IN_ID, "Check in").build(app)?;
    let release_lock = MenuItemBuilder::with_id(TRAY_RELEASE_LOCK_ID, "Release lock").build(app)?;
    let check_updates =
        MenuItemBuilder::with_id(TRAY_CHECK_UPDATES_ID, "Check for updates").build(app)?;
    let quit = MenuItemBuilder::with_id(TRAY_QUIT_ID, "Quit").build(app)?;
    let separator_a = PredefinedMenuItem::separator(app)?;
    let separator_b = PredefinedMenuItem::separator(app)?;

    menu.append_items(&[
        &open,
        &separator_a,
        &sync,
        &check_in,
        &release_lock,
        &separator_b,
        &check_updates,
        &quit,
    ])?;

    Ok(menu)
}

/// Show, unminimize, and focus the main window. Public so the close-to-tray
/// handler and tray menu share one implementation.
pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        window.show()?;
        window.set_focus()?;
    }
    Ok(())
}

fn toggle_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible()? {
            window.hide()?;
        } else {
            show_main_window(app)?;
        }
    }
    Ok(())
}

fn emit_action<R: Runtime>(app: &AppHandle<R>, action: &str) -> tauri::Result<()> {
    app.emit(
        TRAY_ACTION_EVENT,
        TrayActionPayload {
            action: action.to_string(),
        },
    )
}

/// Fire an OS notification for an incoming lock check-in request (SBAI-4044) and
/// surface the window so the holder can act on it. Called from
/// `commands::lock_request_checkin` when a request lands in the local inbox.
///
/// Notification failures are non-fatal (best-effort): if the notification
/// permission is denied or the plugin is unavailable we still raise the window,
/// and the in-app inbox drawer remains the reliable surface.
pub fn notify_lock_request<R: Runtime>(app: &AppHandle<R>, from: &str, path: &str) {
    use tauri_plugin_notification::NotificationExt;

    let body = format!("{from} is asking you to check in {path}");
    let _ = app
        .notification()
        .builder()
        .title("Check-in requested")
        .body(body)
        .show();

    // Bring the window forward so the inbox is visible without hunting for it.
    let _ = show_main_window(app);
}

fn format_title(snapshot: &TraySnapshot) -> String {
    if snapshot.branch.trim().is_empty() {
        return "LoreGUI".to_string();
    }
    format!(
        "{}{}{} dirty",
        snapshot.branch.trim(),
        TRAY_SEPARATOR,
        snapshot.dirty_count
    )
}

fn format_tooltip(snapshot: &TraySnapshot) -> String {
    if snapshot.branch.trim().is_empty() {
        return format!("LoreGUI{}no repository open", TRAY_SEPARATOR);
    }
    format!(
        "LoreGUI{}{}{}{} dirty{}{}",
        TRAY_SEPARATOR,
        snapshot.branch.trim(),
        TRAY_SEPARATOR,
        snapshot.dirty_count,
        TRAY_SEPARATOR,
        snapshot.status.label()
    )
}

fn icon_for_status(status: TrayStatusKind) -> tauri::Result<Image<'static>> {
    let base = Image::from_bytes(include_bytes!("../icons/32x32.png"))?.to_owned();
    let width = base.width();
    let height = base.height();
    let mut rgba = base.rgba().to_vec();
    let dot_x = width as i32 - STATUS_DOT_OFFSET;
    let dot_y = height as i32 - STATUS_DOT_OFFSET;

    draw_dot(
        &mut rgba,
        width as usize,
        height as usize,
        dot_x,
        dot_y,
        STATUS_DOT_OUTER_RADIUS,
        COLOR_STATUS_BORDER,
    );
    draw_dot(
        &mut rgba,
        width as usize,
        height as usize,
        dot_x,
        dot_y,
        STATUS_DOT_INNER_RADIUS,
        status.color(),
    );

    Ok(Image::new_owned(rgba, width, height))
}

fn draw_dot(
    rgba: &mut [u8],
    width: usize,
    height: usize,
    cx: i32,
    cy: i32,
    radius: i32,
    color: [u8; 4],
) {
    let radius_sq = radius * radius;
    for y in (cy - radius).max(0)..=(cy + radius).min(height as i32 - 1) {
        for x in (cx - radius).max(0)..=(cx + radius).min(width as i32 - 1) {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy > radius_sq {
                continue;
            }
            let idx = ((y as usize * width) + x as usize) * 4;
            rgba[idx..idx + 4].copy_from_slice(&color);
        }
    }
}

impl TrayStatusKind {
    fn label(self) -> &'static str {
        match self {
            TrayStatusKind::Clean => "clean",
            TrayStatusKind::Dirty => "dirty",
            TrayStatusKind::Syncing => "syncing",
            TrayStatusKind::Conflict => "conflict",
        }
    }

    fn color(self) -> [u8; 4] {
        match self {
            TrayStatusKind::Clean => COLOR_STATUS_CLEAN,
            TrayStatusKind::Dirty => COLOR_STATUS_DIRTY,
            TrayStatusKind::Syncing => COLOR_STATUS_SYNCING,
            TrayStatusKind::Conflict => COLOR_STATUS_CONFLICT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{format_title, format_tooltip, TraySnapshot, TrayStatusKind};

    #[test]
    fn title_handles_empty_repository() {
        assert_eq!(format_title(&TraySnapshot::default()), "LoreGUI");
    }

    #[test]
    fn title_includes_branch_and_dirty_count() {
        let snapshot = TraySnapshot {
            branch: "main".into(),
            dirty_count: 3,
            status: TrayStatusKind::Dirty,
        };
        assert_eq!(format_title(&snapshot), "main · 3 dirty");
    }

    #[test]
    fn tooltip_includes_status_label() {
        let snapshot = TraySnapshot {
            branch: "release".into(),
            dirty_count: 1,
            status: TrayStatusKind::Syncing,
        };
        assert_eq!(
            format_tooltip(&snapshot),
            "LoreGUI · release · 1 dirty · syncing"
        );
    }
}
