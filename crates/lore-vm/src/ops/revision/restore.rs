//! `revision restore` operation — binds `lore::revision::restore`.
//!
//! Restores the current branch to a previously synced revision, downloading
//! fragments as needed and auto-committing the result. Emits
//! `LoreEvent::RevisionRestoreRevision` carrying the restored revision
//! identifier and number.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::revision::LoreRevisionRestoreArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`restore`].
///
/// Mirrors the `LoreRevisionRestoreArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreArgs {
    /// Commit message for the restored revision.
    #[serde(default)]
    pub message: String,
}

impl RestoreArgs {
    fn into_lore(self) -> LoreRevisionRestoreArgs {
        LoreRevisionRestoreArgs {
            message: LoreString::from_str(&self.message),
            ..Default::default()
        }
    }
}

/// Result returned on a successful restore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    /// BLAKE3 hash signature of the restored revision.
    pub revision: String,
    /// Sequential revision number on the branch.
    pub revision_number: u64,
}

/// Restore the current branch to a previously synced revision.
///
/// Calls the upstream `lore::revision::restore` in-process and collects the
/// `RevisionRestoreRevision` event to return a typed result.
pub async fn restore(api: &LoreApi, args: RestoreArgs) -> Result<RestoreResult> {
    let (callback, rx) = collect_events();

    let status = lore::revision::restore(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision restore failed with status {status}"),
        )));
    }

    let data = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::RevisionRestoreRevision(data) = event {
                Some(data.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            LoreError::Parse(
                "restore succeeded but no RevisionRestoreRevision event emitted".into(),
            )
        })?;

    Ok(RestoreResult {
        revision: format!("{}", data.revision),
        revision_number: data.revision_number,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_args_serializes() {
        let args = RestoreArgs {
            message: "Restore to synced state".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("Restore to synced state"));
    }

    #[test]
    fn restore_args_default_message() {
        let json = "{}";
        let args: RestoreArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.message, "");
    }

    #[test]
    fn restore_args_into_lore_conversion() {
        let args = RestoreArgs {
            message: "Restore revision".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.message.as_str(), "Restore revision");
    }

    #[test]
    fn restore_result_serializes() {
        let result = RestoreResult {
            revision: "abc123def456".into(),
            revision_number: 5,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("abc123def456"));
        assert!(json.contains("5"));
    }
}
