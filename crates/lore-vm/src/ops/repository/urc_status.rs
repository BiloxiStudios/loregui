//! `repository urc_status` operation — the URC status summary (SBAI-5499).
//!
//! Derives a compact recovery/health view from the typed
//! [`super::status::status`] result (no CLI shell-out, no working-tree
//! hashing). The JSON contract is locked for the URC consumer:
//!
//! ```json
//! {
//!   "currentRev": "…",
//!   "remoteRev": "…",
//!   "pendingMerge": false,
//!   "branch": "main",
//!   "diverged": false,
//!   "staged": ["a.txt"],
//!   "conflicts": [],
//!   "healthy": true
//! }
//! ```
//!
//! Mapping rules:
//! - `currentRev` / `remoteRev` — local / remote revision signatures (empty
//!   when the revision event is absent or the remote side is unknown).
//! - `pendingMerge` — a merged (incoming) revision is present.
//! - `diverged` — local is ahead AND remote is ahead.
//! - `staged` / `conflicts` — repository-relative paths of staged /
//!   conflicted files.
//! - `healthy` — `!pendingMerge && !diverged && conflicts.is_empty()`.

use crate::api::LoreApi;
use crate::error::Result;

use serde::{Deserialize, Serialize};

use super::status::{status, RepositoryStatusArgs, RepositoryStatusResult};

/// Arguments for [`urc_status`].
///
/// No options today — the op always runs the underlying status with `staged`
/// reporting enabled so the staged-file list is populated. Kept as a struct so
/// the dispatch contract (`"<domain>.<op>"` + JSON args object) matches every
/// other op and future options stay additive.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UrcStatusArgs {}

/// The URC status summary. Field names serialise camelCase per the locked
/// contract above.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrcStatus {
    /// Local (current) revision signature; empty when none is reported.
    pub current_rev: String,
    /// Remote branch latest revision signature; empty when unknown.
    pub remote_rev: String,
    /// True when a merged (incoming) revision is pending.
    pub pending_merge: bool,
    /// Current branch name.
    pub branch: String,
    /// True when local and remote have both moved ahead of each other.
    pub diverged: bool,
    /// Repository-relative paths of staged files.
    pub staged: Vec<String>,
    /// Repository-relative paths of conflicted (unresolved) files.
    pub conflicts: Vec<String>,
    /// `!pending_merge && !diverged && conflicts.is_empty()`.
    pub healthy: bool,
}

/// Map a typed [`RepositoryStatusResult`] onto the [`UrcStatus`] contract.
///
/// Pure — unit-testable without the engine.
pub fn map_urc_status(result: &RepositoryStatusResult) -> UrcStatus {
    let (current_rev, remote_rev, pending_merge, branch, diverged) = match &result.revision {
        Some(revision) => (
            revision.revision.clone(),
            revision.revision_remote.clone(),
            !revision.revision_merged.is_empty(),
            revision.branch_name.clone(),
            revision.is_local_ahead && revision.is_remote_ahead,
        ),
        None => (String::new(), String::new(), false, String::new(), false),
    };

    let staged: Vec<String> = result
        .files
        .iter()
        .filter(|file| file.staged)
        .map(|file| file.path.clone())
        .collect();
    let conflicts: Vec<String> = result
        .files
        .iter()
        .filter(|file| file.conflict)
        .map(|file| file.path.clone())
        .collect();

    let healthy = !pending_merge && !diverged && conflicts.is_empty();

    UrcStatus {
        current_rev,
        remote_rev,
        pending_merge,
        branch,
        diverged,
        staged,
        conflicts,
        healthy,
    }
}

/// Report the URC status summary for the working directory.
///
/// Runs the in-process [`status`] op with staged reporting enabled and maps
/// its typed result via [`map_urc_status`]. Read-only: no flush is required
/// after this op.
pub async fn urc_status(api: &LoreApi, _args: UrcStatusArgs) -> Result<UrcStatus> {
    let result = status(
        api,
        RepositoryStatusArgs {
            staged: true,
            ..Default::default()
        },
    )
    .await?;
    Ok(map_urc_status(&result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::repository::status::{
        StatusFile, StatusFileAction, StatusNodeType, StatusRevision,
    };
    use serde_json::json;

    fn revision() -> StatusRevision {
        StatusRevision {
            repository: "repo".into(),
            branch: "br".into(),
            branch_name: "main".into(),
            revision: "aaa111".into(),
            revision_number: 7,
            revision_staged: String::new(),
            revision_merged: String::new(),
            revision_remote: "bbb222".into(),
            revision_remote_number: 6,
            is_local_ahead: false,
            is_remote_ahead: false,
            remote_available: true,
            remote_authorized: true,
            remote_branch_exist: true,
        }
    }

    fn file(path: &str, staged: bool, conflict: bool) -> StatusFile {
        StatusFile {
            path: path.into(),
            size: 1,
            action: StatusFileAction::Add,
            node_type: StatusNodeType::File,
            staged,
            conflict,
            dirty: false,
            from_path: String::new(),
        }
    }

    /// The locked contract: exact camelCase field names, in order.
    #[test]
    fn urc_status_serialises_exact_contract_shape() {
        let status = UrcStatus {
            current_rev: "aaa111".into(),
            remote_rev: "bbb222".into(),
            pending_merge: false,
            branch: "main".into(),
            diverged: false,
            staged: vec!["a.txt".into()],
            conflicts: vec![],
            healthy: true,
        };
        let value = serde_json::to_value(&status).expect("should serialize");
        assert_eq!(
            value,
            json!({
                "currentRev": "aaa111",
                "remoteRev": "bbb222",
                "pendingMerge": false,
                "branch": "main",
                "diverged": false,
                "staged": ["a.txt"],
                "conflicts": [],
                "healthy": true,
            })
        );
        let raw = serde_json::to_string(&status).expect("should serialize");
        for key in [
            "currentRev",
            "remoteRev",
            "pendingMerge",
            "branch",
            "diverged",
            "staged",
            "conflicts",
            "healthy",
        ] {
            assert!(raw.contains(&format!("\"{key}\"")), "missing key {key}");
        }
    }

    /// Clean checkout: no merge, no divergence, no conflicts → healthy.
    #[test]
    fn maps_clean_status_as_healthy() {
        let result = RepositoryStatusResult {
            revision: Some(revision()),
            files: vec![file("a.txt", true, false)],
            count: None,
        };
        let status = map_urc_status(&result);
        assert_eq!(status.current_rev, "aaa111");
        assert_eq!(status.remote_rev, "bbb222");
        assert_eq!(status.branch, "main");
        assert!(!status.pending_merge);
        assert!(!status.diverged);
        assert_eq!(status.staged, vec!["a.txt"]);
        assert!(status.conflicts.is_empty());
        assert!(status.healthy);
    }

    /// A pending merged revision flips `pendingMerge` and breaks `healthy`.
    #[test]
    fn pending_merge_breaks_healthy() {
        let mut rev = revision();
        rev.revision_merged = "ccc333".into();
        let result = RepositoryStatusResult {
            revision: Some(rev),
            files: vec![],
            count: None,
        };
        let status = map_urc_status(&result);
        assert!(status.pending_merge);
        assert!(!status.healthy);
    }

    /// Divergence requires BOTH sides ahead; either alone is not diverged.
    #[test]
    fn diverged_requires_local_and_remote_ahead() {
        let mut rev = revision();
        rev.is_local_ahead = true;
        let local_only = map_urc_status(&RepositoryStatusResult {
            revision: Some(rev.clone()),
            files: vec![],
            count: None,
        });
        assert!(!local_only.diverged);
        assert!(local_only.healthy);

        let mut rev = revision();
        rev.is_remote_ahead = true;
        let remote_only = map_urc_status(&RepositoryStatusResult {
            revision: Some(rev),
            files: vec![],
            count: None,
        });
        assert!(!remote_only.diverged);
        assert!(remote_only.healthy);

        let mut rev = revision();
        rev.is_local_ahead = true;
        rev.is_remote_ahead = true;
        let both = map_urc_status(&RepositoryStatusResult {
            revision: Some(rev),
            files: vec![],
            count: None,
        });
        assert!(both.diverged);
        assert!(!both.healthy);
    }

    /// Conflicted files land in `conflicts` and break `healthy`; staged-only
    /// and conflict-only files are partitioned correctly.
    #[test]
    fn conflicts_break_healthy_and_partition_with_staged() {
        let result = RepositoryStatusResult {
            revision: Some(revision()),
            files: vec![
                file("staged.txt", true, false),
                file("conflict.txt", false, true),
                file("both.txt", true, true),
                file("plain.txt", false, false),
            ],
            count: None,
        };
        let status = map_urc_status(&result);
        assert_eq!(status.staged, vec!["staged.txt", "both.txt"]);
        assert_eq!(status.conflicts, vec!["conflict.txt", "both.txt"]);
        assert!(!status.healthy);
    }

    /// No revision event (e.g. an unborn / unreadable repository): all scalar
    /// fields fall back to empty/false and, with no conflicts, the summary is
    /// healthy.
    #[test]
    fn no_revision_edge_defaults_empty() {
        let result = RepositoryStatusResult {
            revision: None,
            files: vec![],
            count: None,
        };
        let status = map_urc_status(&result);
        assert_eq!(status.current_rev, "");
        assert_eq!(status.remote_rev, "");
        assert_eq!(status.branch, "");
        assert!(!status.pending_merge);
        assert!(!status.diverged);
        assert!(status.staged.is_empty());
        assert!(status.conflicts.is_empty());
        assert!(status.healthy);
    }

    /// Args take no options: `{}` deserialises, `null` does not (matching the
    /// dispatch lockstep probe, which routes ops by feeding them `null`).
    #[test]
    fn urc_status_args_defaults() {
        let args: UrcStatusArgs = serde_json::from_str("{}").expect("should deserialize");
        let _ = args;
        assert!(serde_json::from_str::<UrcStatusArgs>("null").is_err());
    }
}
