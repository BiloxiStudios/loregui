//! `repository urc_status` operation — computes the URC cross-crew status contract.
//!
//! Parses lore status and surfaces the TRUE repo state in the shape agreed upon
//! with uefn-mcp (verse-cortex). Field names are a cross-crew contract — LoreGUI
//! + uefn-mcp consume one shape.
//!
//! Output shape (camelCase for frontend compatibility):
//! ```text
//! { currentRev, remoteRev, pendingMerge, branch, diverged, staged[], conflicts[], healthy }
//! ```
//!
//! `healthy = !pendingMerge && !diverged && conflicts.is_empty()`.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreArray, LoreEvent, LoreString};
use lore::repository::LoreRepositoryStatusArgs;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Resolve a path argument against `repo_root` so the upstream engine receives
/// an absolute path. Already-absolute paths pass through unchanged.
fn resolve_path(p: &str, repo_root: &Path) -> LoreString {
    let path = std::path::Path::new(p);
    if path.is_absolute() {
        LoreString::from_str(p)
    } else {
        LoreString::from_path(repo_root.join(path))
    }
}

/// Hash signatures format to all-zero hex when unset; treat that as "empty".
fn hash_or_empty(hash: &lore::interface::Hash) -> String {
    if hash.is_zero() {
        String::new()
    } else {
        format!("{hash}")
    }
}

/// Arguments for [`urc_status`].
///
/// Minimal args — the op always requests sync_point and staged data to compute
/// the full URC contract shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UrcStatusArgs {
    /// Reconcile against the filesystem and refresh dirty tracking.
    /// Enable when the on-disk state may have drifted from tracked dirty flags.
    #[serde(default)]
    pub scan: bool,
    /// Repository-relative paths to limit the status check to; empty checks all.
    #[serde(default)]
    pub paths: Vec<String>,
}

impl UrcStatusArgs {
    fn into_lore(self, repo_root: &Path) -> LoreRepositoryStatusArgs {
        let lore_paths: Vec<LoreString> = self
            .paths
            .iter()
            .map(|p| resolve_path(p, repo_root))
            .collect();
        LoreRepositoryStatusArgs {
            staged: 1,        // Always include staged state
            scan: u8::from(self.scan),
            check_dirty: 0,
            reset: 0,
            sync_point: 1,    // Always include sync point for remote revision
            revision_only: 0,
            count: 0,
            paths: LoreArray::from_vec(lore_paths),
        }
    }
}

/// Result returned on a successful urc_status query.
///
/// This is the cross-crew contract shape — field names must stay stable so
/// LoreGUI + uefn-mcp consume one shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrcStatusResult {
    /// Current local revision hex string (empty when unknown).
    #[serde(rename = "currentRev")]
    pub current_rev: String,
    /// Remote revision hex string (empty when remote unavailable or unknown).
    #[serde(rename = "remoteRev")]
    pub remote_rev: String,
    /// True when there is a pending merge operation awaiting resolution.
    #[serde(rename = "pendingMerge")]
    pub pending_merge: bool,
    /// Current branch name.
    pub branch: String,
    /// True when the local branch has diverged from the remote.
    pub diverged: bool,
    /// Repository-relative paths of staged files.
    pub staged: Vec<String>,
    /// Repository-relative paths of files in conflict.
    pub conflicts: Vec<String>,
    /// True when the repo is in a clean, resolvable state.
    ///
    /// `healthy = !pending_merge && !diverged && conflicts.is_empty()`.
    pub healthy: bool,
}

impl Default for UrcStatusResult {
    fn default() -> Self {
        Self {
            current_rev: String::new(),
            remote_rev: String::new(),
            pending_merge: false,
            branch: String::new(),
            diverged: false,
            staged: Vec::new(),
            conflicts: Vec::new(),
            healthy: true,
        }
    }
}

/// Report the TRUE repo state in the URC cross-crew contract shape.
///
/// Calls `lore::repository::status` with sync_point + staged flags, maps the
/// revision events to the urc_status shape, and derives pendingMerge/diverged/healthy.
pub async fn urc_status(api: &LoreApi, args: UrcStatusArgs) -> Result<UrcStatusResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::repository::status(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("repository status failed with status {status}"),
        )));
    }

    let mut result = UrcStatusResult::default();
    let mut local_ahead = false;
    let mut remote_ahead = false;

    for event in &stream.events {
        match event {
            LoreEvent::RepositoryStatusRevision(data) => {
                result.current_rev = hash_or_empty(&data.revision);
                result.remote_rev = hash_or_empty(&data.revision_remote);
                result.branch = data.branch_name.as_str().to_string();

                // pending_merge: incoming revision is non-zero when a merge is pending
                result.pending_merge = !data.revision_merged.is_zero();

                // diverged: both local_ahead and remote_ahead are non-zero
                local_ahead = data.is_local_ahead != 0;
                remote_ahead = data.is_remote_ahead != 0;
            }
            LoreEvent::RepositoryStatusFile(data) => {
                if data.flag_staged != 0 {
                    result.staged.push(data.path.as_str().to_string());
                }
                if data.flag_conflict != 0 {
                    result.conflicts.push(data.path.as_str().to_string());
                }
            }
            _ => {}
        }
    }

    result.diverged = local_ahead && remote_ahead;
    result.healthy = !result.pending_merge && !result.diverged && result.conflicts.is_empty();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urc_status_args_defaults() {
        let json = r#"{}"#;
        let args: UrcStatusArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(!args.scan);
        assert!(args.paths.is_empty());
    }

    #[test]
    fn urc_status_args_with_scan_and_paths() {
        let json = r#"{"scan":true,"paths":["a.txt","src/b.rs"]}"#;
        let args: UrcStatusArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(args.scan);
        assert_eq!(args.paths.len(), 2);
        assert_eq!(args.paths[0], "a.txt");
    }

    #[test]
    fn urc_status_args_into_lore_sets_sync_point_and_staged() {
        let args = UrcStatusArgs {
            scan: true,
            paths: vec!["src/main.rs".into()],
        };
        let repo_root = std::path::Path::new("/work/myrepo");
        let lore_args = args.into_lore(repo_root);
        assert_eq!(lore_args.staged, 1, "staged must always be 1");
        assert_eq!(lore_args.scan, 1);
        assert_eq!(lore_args.sync_point, 1, "sync_point must always be 1");
        assert_eq!(lore_args.paths.len(), 1);
    }

    #[test]
    fn urc_status_result_serializes_camelcase() {
        let result = UrcStatusResult {
            current_rev: "abc123".into(),
            remote_rev: "def456".into(),
            pending_merge: true,
            branch: "main-abc123".into(),
            diverged: true,
            staged: vec!["foo.txt".into()],
            conflicts: vec!["bar.txt".into()],
            healthy: false,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        // Verify camelCase keys
        assert!(json.contains(r#""currentRev""#));
        assert!(json.contains(r#""remoteRev""#));
        assert!(json.contains(r#""pendingMerge":true"#));
        assert!(json.contains(r#""diverged":true"#));
        assert!(json.contains(r#""healthy":false"#));
        assert!(json.contains("abc123"));
        assert!(json.contains("def456"));
        assert!(json.contains("foo.txt"));
        assert!(json.contains("bar.txt"));
    }

    #[test]
    fn urc_status_result_defaults_to_healthy() {
        let result = UrcStatusResult::default();
        assert!(result.healthy, "default result should be healthy");
        assert!(result.staged.is_empty());
        assert!(result.conflicts.is_empty());
        assert!(!result.pending_merge);
        assert!(!result.diverged);
    }

    #[test]
    fn healthy_computed_correctly_clean_repo() {
        let result = UrcStatusResult {
            current_rev: "abc".into(),
            remote_rev: "abc".into(),
            pending_merge: false,
            branch: "main".into(),
            diverged: false,
            staged: vec![],
            conflicts: vec![],
            healthy: true,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""healthy":true"#));
    }

    #[test]
    fn healthy_false_when_pending_merge() {
        let result = UrcStatusResult {
            current_rev: "abc".into(),
            remote_rev: "abc".into(),
            pending_merge: true,
            branch: "main".into(),
            diverged: false,
            staged: vec![],
            conflicts: vec![],
            healthy: false,
        };
        assert!(!result.healthy);
    }

    #[test]
    fn healthy_false_when_diverged() {
        let result = UrcStatusResult {
            current_rev: "abc".into(),
            remote_rev: "def".into(),
            pending_merge: false,
            branch: "main-abc".into(),
            diverged: true,
            staged: vec![],
            conflicts: vec![],
            healthy: false,
        };
        assert!(!result.healthy);
    }

    #[test]
    fn healthy_false_when_conflicts() {
        let result = UrcStatusResult {
            current_rev: "abc".into(),
            remote_rev: "abc".into(),
            pending_merge: false,
            branch: "main".into(),
            diverged: false,
            staged: vec![],
            conflicts: vec!["conflict.txt".into()],
            healthy: false,
        };
        assert!(!result.healthy);
    }

    /// Regression: relative paths are resolved against repo_root.
    #[test]
    fn urc_status_args_resolves_relative_paths() {
        let args = UrcStatusArgs {
            paths: vec!["src/main.rs".into()],
            ..Default::default()
        };
        let repo_root = std::path::Path::new("/work/myrepo");
        let lore_args = args.into_lore(repo_root);
        assert_eq!(
            lore_args.paths.as_slice()[0].as_str(),
            "/work/myrepo/src/main.rs"
        );
    }

    /// Roundtrip: serialize then deserialize must preserve all fields.
    #[test]
    fn urc_status_result_roundtrip() {
        let result = UrcStatusResult {
            current_rev: "c1".into(),
            remote_rev: "c2".into(),
            pending_merge: false,
            branch: "feature".into(),
            diverged: true,
            staged: vec!["a.txt".into(), "b.txt".into()],
            conflicts: vec!["c.txt".into()],
            healthy: false,
        };
        let json = serde_json::to_string(&result).expect("serialize");
        let back: UrcStatusResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.current_rev, "c1");
        assert_eq!(back.remote_rev, "c2");
        assert!(!back.pending_merge);
        assert_eq!(back.branch, "feature");
        assert!(back.diverged);
        assert_eq!(back.staged.len(), 2);
        assert_eq!(back.conflicts.len(), 1);
        assert!(!back.healthy);
    }
}
