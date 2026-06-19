//! `shared_store set_use_automatically` operation — binds `lore::shared_store::set_use_automatically`.
//!
//! Sets whether the configured default shared store should be used automatically.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::shared_store::LoreSharedStoreSetUseAutomaticallyArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`set_use_automatically`].
///
/// Mirrors `LoreSharedStoreSetUseAutomaticallyArgs` from the upstream `lore` crate
/// but uses plain `bool` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetUseAutomaticallyArgs {
    /// Whether to automatically use the default shared store.
    pub enabled: bool,
}

impl SetUseAutomaticallyArgs {
    fn into_lore(self) -> LoreSharedStoreSetUseAutomaticallyArgs {
        LoreSharedStoreSetUseAutomaticallyArgs {
            enabled: if self.enabled { 1 } else { 0 },
        }
    }
}

/// Result returned on success — empty, as no domain-specific events are emitted.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SetUseAutomaticallyResult;

/// Set whether to automatically use the configured default shared store.
///
/// Calls the upstream `lore::shared_store::set_use_automatically` in-process and
/// returns a typed result on success.
pub async fn set_use_automatically(
    api: &LoreApi,
    args: SetUseAutomaticallyArgs,
) -> Result<SetUseAutomaticallyResult> {
    let (callback, rx) = collect_events();

    let status = lore::shared_store::set_use_automatically(
        api.globals().build(),
        args.into_lore(),
        callback,
    )
    .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("set_use_automatically failed with status {status}"),
        )));
    }

    Ok(SetUseAutomaticallyResult)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes_cleanly() {
        let args = SetUseAutomaticallyArgs { enabled: true };
        let json = serde_json::to_string(&args).unwrap();
        assert!(json.contains("true"));

        let args_false = SetUseAutomaticallyArgs { enabled: false };
        let json_false = serde_json::to_string(&args_false).unwrap();
        assert!(json_false.contains("false"));
    }

    #[test]
    fn into_lore_maps_bool_to_u8() {
        let args_true = SetUseAutomaticallyArgs { enabled: true };
        let lore_args = args_true.into_lore();
        assert_eq!(lore_args.enabled, 1);

        let args_false = SetUseAutomaticallyArgs { enabled: false };
        let lore_args = args_false.into_lore();
        assert_eq!(lore_args.enabled, 0);
    }
}
