//! `branch metadata_clear` operation — binds `lore::branch::metadata_clear`.
//!
//! Clears metadata keys from a branch. Use `metadata_get` to read keys and
//! `metadata_set` to write them; `metadata_clear` removes them entirely.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchMetadataClearArgs;
use lore::interface::LoreString;
use serde::{Deserialize, Serialize};

/// Arguments for [`metadata_clear`].
///
/// Mirrors `LoreBranchMetadataClearArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataClearArgs {
    /// Branch name; empty string uses the current branch.
    #[serde(default)]
    pub branch: String,
    /// Metadata keys to clear (e.g., "description", "owner").
    /// Can clear multiple keys at once.
    pub keys: Vec<String>,
}

impl MetadataClearArgs {
    fn into_lore(self) -> LoreBranchMetadataClearArgs {
        LoreBranchMetadataClearArgs {
            branch: LoreString::from_str(&self.branch),
            keys: lore::interface::LoreArray::from_vec(
                self.keys
                    .into_iter()
                    .map(|k| LoreString::from_str(&k))
                    .collect(),
            ),
        }
    }
}

/// Result returned on successful metadata clear.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataClearResult {
    /// The branch whose metadata was cleared.
    pub branch: String,
    /// The keys that were cleared.
    pub keys: Vec<String>,
}

/// Clear metadata keys from a branch.
///
/// Calls the upstream `lore::branch::metadata_clear` in-process and returns
/// a typed result indicating which keys were cleared from which branch.
pub async fn metadata_clear(
    api: &LoreApi,
    args: MetadataClearArgs,
) -> Result<MetadataClearResult> {
    let branch = args.branch.clone();
    let keys = args.keys.clone();

    let (callback, rx) = collect_events();

    let status = lore::branch::metadata_clear(
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
            || format!("metadata_clear failed with status {status}"),
        )));
    }

    // The metadata_clear operation doesn't emit a specific event with the
    // cleared keys; we just return success with the args we were given.
    Ok(MetadataClearResult { branch, keys })
}
