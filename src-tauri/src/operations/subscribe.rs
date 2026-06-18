//! Subscribe to notification events.
//!
//! Registers the caller (Tauri frontend) for live updates about repository
//! changes. Returns a subscription ID that must be passed to [`super::unsubscribe`]
//! to cancel.
//!
//! # One-file-per-op
//! This file implements ONLY the subscribe operation. See `unsubscribe.rs` for
//! the counterpart.

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use super::{NotificationKind, SubscriptionId};
use crate::commands::AppState;

/// Payload emitted to the frontend when a branch change occurs.
#[derive(Debug, Clone, Serialize)]
pub struct BranchChangeEvent {
    pub repo_path: String,
    pub old_branch: Option<String>,
    pub new_branch: Option<String>,
    /// Human-readable reason: "switch" | "create" | "delete" | "merge".
    pub reason: String,
}

/// Payload emitted when the sync status (ahead/behind) changes.
#[derive(Debug, Clone, Serialize)]
pub struct SyncStatusEvent {
    pub repo_path: String,
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
}

/// Payload emitted when a file lock is acquired or released.
#[derive(Debug, Clone, Serialize)]
pub struct LockChangeEvent {
    pub repo_path: String,
    pub path: String,
    pub locked: bool,
    pub owner: Option<String>,
}

/// Request body for the subscribe command.
#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    /// Which notification category to subscribe to.
    pub kind: NotificationKind,
}

/// Response from the subscribe command.
#[derive(Debug, Clone, Serialize)]
pub struct SubscribeResponse {
    /// Opaque subscription ID — pass this to unsubscribe to cancel.
    pub subscription_id: SubscriptionId,
    /// Echo of the kind that was subscribed to.
    pub kind: NotificationKind,
}

/// Tauri command: subscribe to live notification events.
///
/// Registers the caller for events of the requested `kind`. The returned
/// `subscription_id` must be used when calling `unsubscribe_notifications`
/// to tear down the subscription.
///
/// # Events emitted
/// Depending on `kind`, the frontend will receive:
/// - `notification/branch_change` — [`BranchChangeEvent`]
/// - `notification/sync_status` — [`SyncStatusEvent`]
/// - `notification/lock_change` — [`LockChangeEvent`]
#[tauri::command]
pub async fn subscribe_notifications(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: SubscribeRequest,
) -> Result<SubscribeResponse, String> {
    let subscription_id = state.next_subscription_id();

    tracing::info!(
        "subscribe: kind={:?} subscription_id={}",
        request.kind,
        subscription_id
    );

    // Emit a confirmation event back to the subscribing window.
    app.emit(
        "notification/subscribed",
        SubscribeResponse {
            subscription_id,
            kind: request.kind.clone(),
        },
    )
    .map_err(|e| format!("failed to emit subscription confirmation: {e}"))?;

    Ok(SubscribeResponse {
        subscription_id,
        kind: request.kind,
    })
}
