//! `revision commit` operation — binds `lore::revision::commit`.
//!
//! Commits all staged changes to the current branch as a new revision. Emits
//! `LoreEvent::RevisionCommitRevision` carrying the new revision identifier,
//! number, and branch.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::revision::LoreRevisionCommitArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`commit`].
///
/// Mirrors the happy-path subset of `LoreRevisionCommitArgs` from the upstream
/// `lore` crate. Link- and layer-specific fields take their upstream defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitArgs {
    /// Commit message describing the revision.
    pub message: String,
}

impl CommitArgs {
    fn into_lore(self) -> LoreRevisionCommitArgs {
        LoreRevisionCommitArgs {
            message: LoreString::from_str(&self.message),
            ..Default::default()
        }
    }
}

/// Result returned on a successful commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitResult {
    /// BLAKE3 hash signature of the newly created revision.
    pub revision: String,
    /// Sequential revision number on the branch.
    pub revision_number: u64,
    /// Branch identifier the revision was committed on.
    pub branch: String,
}

/// Commit staged changes as a new revision.
///
/// Calls the upstream `lore::revision::commit` in-process and collects the
/// `RevisionCommitRevision` event to return a typed result.
pub async fn commit(api: &LoreApi, args: CommitArgs) -> Result<CommitResult> {
    let (callback, rx) = collect_events();

    let status = lore::revision::commit(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision commit failed with status {status}"),
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
            LoreError::Parse("commit succeeded but no RevisionCommitRevision event emitted".into())
        })?;

    Ok(CommitResult {
        revision: format!("{}", data.revision),
        revision_number: data.revision_number,
        branch: format!("{}", data.branch),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_args_serializes() {
        let args = CommitArgs {
            message: "Initial commit".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("Initial commit"));
    }

    #[test]
    fn commit_args_into_lore_conversion() {
        let args = CommitArgs {
            message: "Add feature".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.message.as_str(), "Add feature");
    }

    #[test]
    fn commit_result_serializes() {
        let result = CommitResult {
            revision: "abc123".into(),
            revision_number: 1,
            branch: "main".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("abc123"));
        assert!(json.contains("main"));
    }
}
