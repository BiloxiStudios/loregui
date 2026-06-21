//! `notification subscribe` operation — binds `lore::notification::subscribe`.
//!
//! Subscribes to repository push-event notifications. The subscription remains
//! active until a corresponding [`unsubscribe`](super::unsubscribe) call.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::notification::LoreNotificationSubscribeArgs;
use serde::{Deserialize, Serialize};

/// Result returned on successful subscribe.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubscribeResult {}

/// Subscribe to repository notifications.
///
/// Calls the upstream `lore::notification::subscribe` in-process.
/// Returns `Ok(SubscribeResult)` when the subscribe succeeds.
pub async fn subscribe(api: &LoreApi) -> Result<SubscribeResult> {
    let (callback, rx) = collect_events();

    let status = lore::notification::subscribe(
        api.globals().build(),
        LoreNotificationSubscribeArgs {},
        callback,
    )
    .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("subscribe failed with status {status}"),
        )));
    }

    Ok(SubscribeResult::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serializes() {
        let result = SubscribeResult::default();
        let json = serde_json::to_string(&result).expect("serialise");
        assert_eq!(json, "{}");
    }
}
