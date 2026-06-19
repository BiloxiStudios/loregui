//! `revision amend` operation — binds `lore::revision::amend`.
//!
//! Amends the most recent revision by updating its commit message. Emits
//! `LoreEvent::RevisionCommitRevision` carrying the amended revision details.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::revision::LoreRevisionAmendArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`amend`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmendArgs {
    /// New commit message for the most recent revision.
    pub message: String,
}

impl AmendArgs {
    fn into_lore(self) -> LoreRevisionAmendArgs {
        LoreRevisionAmendArgs {
            message: LoreString::from_str(&self.message),
        }
    }
}

/// Result returned on a successful amend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmendResult {
    /// BLAKE3 hash signature of the amended revision.
    pub revision: String,
    /// Sequential revision number on the branch.
    pub revision_number: u64,
    /// Branch identifier the revision belongs to.
    pub branch: String,
}

/// Amend the most recent revision's commit message.
///
/// Calls the upstream `lore::revision::amend` in-process and collects the
/// `RevisionCommitRevision` event to return a typed result.
pub async fn amend(api: &LoreApi, args: AmendArgs) -> Result<AmendResult> {
    let (callback, rx) = collect_events();

    let status = lore::revision::amend(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision amend failed with status {status}"),
        )));
    }

    let data = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::RevisionCommitRevision(data) = event {
                Some(data.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            LoreError::Parse("amend succeeded but no RevisionCommitRevision event emitted".into())
        })?;

    Ok(AmendResult {
        revision: format!("{}", data.revision),
        revision_number: data.revision_number,
        branch: format!("{}", data.branch),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amend_args_serializes() {
        let args = AmendArgs {
            message: "Updated message".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("Updated message"));
    }

    #[test]
    fn amend_args_into_lore_conversion() {
        let args = AmendArgs {
            message: "Fix typo".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.message.as_str(), "Fix typo");
    }

    #[test]
    fn amend_result_serializes() {
        let result = AmendResult {
            revision: "abc123".into(),
            revision_number: 2,
            branch: "main".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("abc123"));
        assert!(json.contains("main"));
    }
}
