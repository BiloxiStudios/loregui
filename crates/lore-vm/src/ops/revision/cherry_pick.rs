//! `revision cherry_pick` operation — binds `lore::revision::cherry_pick`.
//!
//! Cherry-picks a revision onto the current branch. If no conflicts arise and
//! `no_commit` is false, the operation auto-commits and emits a
//! `RevisionCommitRevision` event with the new revision details. If conflicts
//! arise, the cherry-pick enters an in-progress state that must be resolved
//! (via `cherry_pick_resolve*`) or aborted (`cherry_pick_abort`).

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::revision::LoreRevisionCherryPickArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`cherry_pick`].
///
/// Mirrors `LoreRevisionCherryPickArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CherryPickArgs {
    /// Revision identifier to cherry-pick (hash, branch name, or symbolic ref).
    pub revision: String,
    /// Commit message for the auto-commit when no conflicts arise.
    /// Ignored when `no_commit` is true.
    #[serde(default)]
    pub message: String,
    /// When true, stage the cherry-picked changes but do not auto-commit,
    /// even if no conflicts arise.
    #[serde(default)]
    pub no_commit: bool,
}

impl CherryPickArgs {
    fn into_lore(self) -> LoreRevisionCherryPickArgs {
        LoreRevisionCherryPickArgs {
            revision: LoreString::from_str(&self.revision),
            message: LoreString::from_str(&self.message),
            no_commit: u8::from(self.no_commit),
        }
    }
}

/// Result returned on a successful cherry-pick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CherryPickResult {
    /// If the cherry-pick auto-committed, the BLAKE3 hash of the new revision.
    /// `None` when `no_commit` was set or conflicts arose.
    pub revision: Option<String>,
    /// Sequential revision number on the branch (present only on auto-commit).
    pub revision_number: Option<u64>,
    /// Branch the revision was committed on (present only on auto-commit).
    pub branch: Option<String>,
}

/// Cherry-pick a revision onto the current branch.
///
/// Calls the upstream `lore::revision::cherry_pick` in-process and collects
/// events. On a clean cherry-pick with auto-commit, returns the new revision
/// details. On conflicts or `no_commit`, the revision fields are `None`.
pub async fn cherry_pick(api: &LoreApi, args: CherryPickArgs) -> Result<CherryPickResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::revision::cherry_pick(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("cherry_pick failed with status {status}"),
        )));
    }

    // If the cherry-pick auto-committed, extract the revision details.
    let commit_data = stream.events.iter().find_map(|event| {
        if let LoreEvent::RevisionCommitRevision(data) = event {
            Some(data.clone())
        } else {
            None
        }
    });

    Ok(CherryPickResult {
        revision: commit_data.as_ref().map(|d| format!("{}", d.revision)),
        revision_number: commit_data.as_ref().map(|d| d.revision_number),
        branch: commit_data.as_ref().map(|d| format!("{}", d.branch)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises() {
        let args = CherryPickArgs {
            revision: "abc123".into(),
            message: "Cherry-pick fix".into(),
            no_commit: false,
        };
        let json = serde_json::to_string(&args).expect("should serialise");
        assert!(json.contains("abc123"));
        assert!(json.contains("Cherry-pick fix"));
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"revision":"def456","message":"pick it","no_commit":true}"#;
        let args: CherryPickArgs = serde_json::from_str(json).expect("should deserialise");
        assert_eq!(args.revision, "def456");
        assert_eq!(args.message, "pick it");
        assert!(args.no_commit);
    }

    #[test]
    fn args_defaults() {
        let json = r#"{"revision":"abc"}"#;
        let args: CherryPickArgs = serde_json::from_str(json).expect("should deserialise");
        assert_eq!(args.revision, "abc");
        assert_eq!(args.message, "");
        assert!(!args.no_commit);
    }

    #[test]
    fn args_into_lore_conversion() {
        let args = CherryPickArgs {
            revision: "rev1".into(),
            message: "msg".into(),
            no_commit: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.revision.as_str(), "rev1");
        assert_eq!(lore_args.message.as_str(), "msg");
        assert_eq!(lore_args.no_commit, 1);
    }

    #[test]
    fn args_into_lore_no_commit_false() {
        let args = CherryPickArgs {
            revision: "rev2".into(),
            message: "".into(),
            no_commit: false,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.no_commit, 0);
    }

    #[test]
    fn result_with_commit_serialises() {
        let result = CherryPickResult {
            revision: Some("abc123".into()),
            revision_number: Some(5),
            branch: Some("main".into()),
        };
        let json = serde_json::to_string(&result).expect("should serialise");
        assert!(json.contains("abc123"));
        assert!(json.contains("main"));
    }

    #[test]
    fn result_without_commit_serialises() {
        let result = CherryPickResult {
            revision: None,
            revision_number: None,
            branch: None,
        };
        let json = serde_json::to_string(&result).expect("should serialise");
        assert!(json.contains("null"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"revision":"x","revision_number":1,"branch":"dev"}"#;
        let result: CherryPickResult = serde_json::from_str(json).expect("should deserialise");
        assert_eq!(result.revision.as_deref(), Some("x"));
        assert_eq!(result.revision_number, Some(1));
        assert_eq!(result.branch.as_deref(), Some("dev"));
    }

    #[test]
    fn result_deserialises_nulls() {
        let json = r#"{"revision":null,"revision_number":null,"branch":null}"#;
        let result: CherryPickResult = serde_json::from_str(json).expect("should deserialise");
        assert!(result.revision.is_none());
        assert!(result.revision_number.is_none());
        assert!(result.branch.is_none());
    }
}
