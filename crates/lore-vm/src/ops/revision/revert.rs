//! `revision revert` operation — binds `lore::revision::revert`.
//!
//! Reverts the working directory to a specified revision by applying the
//! inverse of its changes, with remote dispatch. Unlike `revert_local`, this
//! variant can dispatch the operation to a remote shared store. Supports
//! optional auto-commit when no conflicts arise.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::revision::LoreRevisionRevertArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`revert`].
///
/// Mirrors `LoreRevisionRevertArgs` from the upstream `lore` crate
/// but uses plain Rust types so it serialises cleanly across the Tauri
/// boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertArgs {
    /// The revision to revert.
    pub revision: String,
    /// Commit message for the auto-commit when no conflicts arise.
    #[serde(default)]
    pub message: String,
    /// When true, skip auto-commit even if there are no conflicts.
    #[serde(default)]
    pub no_commit: bool,
}

impl RevertArgs {
    fn into_lore(self) -> LoreRevisionRevertArgs {
        LoreRevisionRevertArgs {
            revision: LoreString::from_str(&self.revision),
            message: LoreString::from_str(&self.message),
            no_commit: u8::from(self.no_commit),
        }
    }
}

/// A file that has an unresolved conflict after the revert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertConflictFile {
    /// Repository-relative path of the conflicted file.
    pub path: String,
}

/// Result returned on successful revert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertResult {
    /// Whether the revert produced conflicts that require manual resolution.
    pub has_conflicts: bool,
    /// Files with unresolved conflicts (empty when `has_conflicts` is false).
    pub conflict_files: Vec<RevertConflictFile>,
    /// The revision hash of the auto-commit, if one was created.
    pub committed_revision: Option<String>,
}

/// Revert the working directory to a specified revision (with remote dispatch).
///
/// Calls the upstream `lore::revision::revert` in-process and collects
/// revert events into a typed result.
pub async fn revert(api: &LoreApi, args: RevertArgs) -> Result<RevertResult> {
    let (callback, rx) = collect_events();

    let status = lore::revision::revert(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(
            stream
                .error
                .unwrap_or_else(|| format!("revert failed with status {status}")),
        ));
    }

    let mut has_conflicts = false;
    let mut conflict_files = Vec::new();
    let mut committed_revision = None;

    for event in &stream.events {
        match event {
            LoreEvent::RevertStartEnd(data) => {
                has_conflicts = data.has_conflicts != 0;
            }
            LoreEvent::RevertConflictFile(data) => {
                conflict_files.push(RevertConflictFile {
                    path: data.path.as_str().to_string(),
                });
            }
            LoreEvent::RevisionCommitRevision(data) => {
                committed_revision = Some(format!("{}", data.revision));
            }
            _ => {}
        }
    }

    Ok(RevertResult {
        has_conflicts,
        conflict_files,
        committed_revision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises() {
        let args = RevertArgs {
            revision: "abc123".into(),
            message: "Revert bad change".into(),
            no_commit: false,
        };
        let json = serde_json::to_string(&args).expect("should serialise");
        assert!(json.contains("abc123"));
        assert!(json.contains("Revert bad change"));
    }

    #[test]
    fn args_deserialises_with_defaults() {
        let json = r#"{"revision": "abc123"}"#;
        let args: RevertArgs = serde_json::from_str(json).expect("should deserialise");
        assert_eq!(args.revision, "abc123");
        assert_eq!(args.message, "");
        assert!(!args.no_commit);
    }

    #[test]
    fn args_into_lore_conversion() {
        let args = RevertArgs {
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
    fn args_no_commit_false() {
        let args = RevertArgs {
            revision: "rev1".into(),
            message: "".into(),
            no_commit: false,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.no_commit, 0);
    }

    #[test]
    fn result_serialises() {
        let result = RevertResult {
            has_conflicts: true,
            conflict_files: vec![RevertConflictFile {
                path: "src/main.rs".into(),
            }],
            committed_revision: None,
        };
        let json = serde_json::to_string(&result).expect("should serialise");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("true"));
    }

    #[test]
    fn result_no_conflicts() {
        let result = RevertResult {
            has_conflicts: false,
            conflict_files: vec![],
            committed_revision: Some("deadbeef".into()),
        };
        let json = serde_json::to_string(&result).expect("should serialise");
        assert!(json.contains("deadbeef"));
        assert!(!json.contains("true"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"has_conflicts":false,"conflict_files":[],"committed_revision":"abc"}"#;
        let result: RevertResult = serde_json::from_str(json).expect("should deserialise");
        assert!(!result.has_conflicts);
        assert!(result.conflict_files.is_empty());
        assert_eq!(result.committed_revision.as_deref(), Some("abc"));
    }

    #[test]
    fn conflict_file_serialises() {
        let cf = RevertConflictFile {
            path: "test.txt".into(),
        };
        let json = serde_json::to_string(&cf).expect("should serialise");
        assert!(json.contains("test.txt"));
    }
}
