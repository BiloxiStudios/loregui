//! Unsubscribe from notification events.
//!
//! Tears down an active subscription created by [`super::subscribe`].
//!
//! # One-file-per-op
//! This file implements ONLY the unsubscribe operation. See `subscribe.rs` for
//! the counterpart.

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use super::SubscriptionId;
use crate::commands::AppState;

/// Request body for the unsubscribe command.
#[derive(Debug, Deserialize)]
pub struct UnsubscribeRequest {
    /// The subscription ID returned by `subscribe_notifications`.
    pub subscription_id: SubscriptionId,
}

/// Response from the unsubscribe command.
#[derive(Debug, Serialize)]
pub struct UnsubscribeResponse {
    /// The subscription that was removed.
    pub subscription_id: SubscriptionId,
    /// True if the subscription existed and was removed; false if it was
    /// already gone (idempotent — callers can safely retry).
    pub was_active: bool,
}

/// Tauri command: unsubscribe from live notification events.
///
/// Tears down the subscription identified by `subscription_id`. This operation
/// is idempotent — calling it twice with the same ID is not an error.
#[tauri::command]
pub async fn unsubscribe_notifications(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: UnsubscribeRequest,
) -> Result<UnsubscribeResponse, String> {
    let was_active = state.remove_subscription(request.subscription_id);

    tracing::info!(
        "unsubscribe: subscription_id={} was_active={}",
        request.subscription_id,
        was_active
    );

    // Emit a confirmation event.
    app.emit(
        "notification/unsubscribed",
        UnsubscribeResponse {
            subscription_id: request.subscription_id,
            was_active,
        },
    )
    .map_err(|e| format!("failed to emit unsubscription confirmation: {e}"))?;

    Ok(UnsubscribeResponse {
        subscription_id: request.subscription_id,
        was_active,
    })
}
