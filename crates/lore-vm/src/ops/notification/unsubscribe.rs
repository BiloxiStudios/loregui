//! `notification unsubscribe` operation — binds `lore::notification::unsubscribe`.
//!
//! Unsubscribes from repository push-event notifications that were established
//! by a prior [`subscribe`](super::subscribe) call.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::notification::LoreNotificationUnsubscribeArgs;
use serde::{Deserialize, Serialize};

/// Result returned on successful unsubscribe.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UnsubscribeResult {}

/// Unsubscribe from repository notifications.
///
/// Calls the upstream `lore::notification::unsubscribe` in-process.
/// Returns `Ok(UnsubscribeResult)` when the unsubscribe succeeds.
pub async fn unsubscribe(api: &LoreApi) -> Result<UnsubscribeResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::notification::unsubscribe(api.globals().build(), LoreNotificationUnsubscribeArgs {}, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(
            stream
                .error
                .unwrap_or_else(|| format!("unsubscribe failed with status {status}")),
        ));
    }

    Ok(UnsubscribeResult::default())
}
