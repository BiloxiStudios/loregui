//! `auth::clear` — clears all stored authentication identities and tokens.
//!
//! Binds [`lore::auth::clear`] in-process (no CLI shelling).
//! Emits no result events on success; returns `Result<()>`.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};
use serde::{Deserialize, Serialize};

/// Arguments for [`clear`].
///
/// Empty struct — the upstream `lore::auth::clear` takes no meaningful arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearArgs {}

/// Clears all stored authentication identities and tokens.
///
/// Calls the upstream `lore::auth::clear` in-process and waits for completion.
/// Returns success if the operation completed without errors.
pub async fn clear(api: &LoreApi, _args: ClearArgs) -> Result<()> {
    let (callback, rx) = collect_events();

    let status = lore::auth::clear(
        api.globals().build(),
        lore::auth::LoreAuthClearArgs::default(),
        callback,
    )
    .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(
            stream
                .error
                .unwrap_or_else(|| format!("clear failed with status {status}")),
        ));
    }

    Ok(())
}
