//! `repository status` operation — binds `lore::repository::status`.
//!
//! Reports the working-directory status: the current/staged revision plus the
//! set of files with pending changes. Emits `RepositoryStatusRevision` (branch
//! and revision context), `RepositoryStatusFile` per changed file, and an
//! optional `RepositoryStatusCount` summary when `count` is set.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::repository::LoreRepositoryStatusArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`status`].
///
/// Mirrors `LoreRepositoryStatusArgs` from the upstream `lore` crate but uses
/// plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepositoryStatusArgs {
    /// Include staged state in the report.
    #[serde(default)]
    pub staged: bool,
    /// Reconcile against the filesystem and refresh dirty tracking.
    #[serde(default)]
    pub scan: bool,
    /// Verify dirty flags against the filesystem without a full scan.
    #[serde(default)]
    pub check_dirty: bool,
    /// Reset the tracked state before computing status.
    #[serde(default)]
    pub reset: bool,
    /// Include the sync point in the report.
    #[serde(default)]
    pub sync_point: bool,
    /// Only emit revision info, skipping all diffs.
    #[serde(default)]
    pub revision_only: bool,
    /// Count directories and files in the staged state / current revision.
    #[serde(default)]
    pub count: bool,
    /// Repository-relative paths to limit the status to; empty checks all.
    #[serde(default)]
    pub paths: Vec<String>,
}

impl RepositoryStatusArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreRepositoryStatusArgs {
        LoreRepositoryStatusArgs {
            staged: u8::from(self.staged),
            scan: u8::from(self.scan),
            check_dirty: u8::from(self.check_dirty),
            reset: u8::from(self.reset),
            sync_point: u8::from(self.sync_point),
            revision_only: u8::from(self.revision_only),
            count: u8::from(self.count),
            paths: crate::ops::paths::lore_path_args(repo_root, &self.paths),
        }
    }
}

/// The action applied to a file reported by status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusFileAction {
    Keep,
    Add,
    Delete,
    Move,
    Copy,
}

/// The kind of node reported by status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusNodeType {
    Directory,
    File,
    Link,
}

/// One file reported by status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusFile {
    /// Repository-relative path.
    pub path: String,
    /// Size of the file in bytes.
    pub size: u64,
    /// Change applied to the file.
    pub action: StatusFileAction,
    /// Node kind.
    pub node_type: StatusNodeType,
    /// True when the change is staged.
    pub staged: bool,
    /// True when the file is in conflict.
    pub conflict: bool,
    /// True when the file differs from the recorded state.
    pub dirty: bool,
    /// Previous path when moved/copied; empty otherwise.
    pub from_path: String,
}

/// Revision/branch context reported by status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusRevision {
    /// Repository identifier.
    pub repository: String,
    /// Current branch identifier.
    pub branch: String,
    /// Current branch name.
    pub branch_name: String,
    /// Current revision identifier.
    pub revision: String,
    /// Current revision number.
    pub revision_number: u64,
    /// Staged revision identifier (empty when nothing is staged).
    pub revision_staged: String,
}

/// Count summary reported by status when `count` is set.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusCount {
    pub directories: u64,
    pub files: u64,
}

/// Result returned on a successful status query.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepositoryStatusResult {
    /// Revision/branch context, when reported.
    pub revision: Option<StatusRevision>,
    /// One entry per file with pending changes.
    pub files: Vec<StatusFile>,
    /// Count summary, when `count` was requested.
    pub count: Option<StatusCount>,
}

fn map_action(action: &lore::interface::LoreFileAction) -> StatusFileAction {
    match action {
        lore::interface::LoreFileAction::Keep => StatusFileAction::Keep,
        lore::interface::LoreFileAction::Add => StatusFileAction::Add,
        lore::interface::LoreFileAction::Delete => StatusFileAction::Delete,
        lore::interface::LoreFileAction::Move => StatusFileAction::Move,
        lore::interface::LoreFileAction::Copy => StatusFileAction::Copy,
    }
}

fn map_node_type(node_type: &lore::interface::LoreNodeType) -> StatusNodeType {
    match node_type {
        lore::interface::LoreNodeType::Directory => StatusNodeType::Directory,
        lore::interface::LoreNodeType::File => StatusNodeType::File,
        lore::interface::LoreNodeType::Link => StatusNodeType::Link,
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

/// Report the working-directory status.
///
/// Calls the upstream `lore::repository::status` in-process and collects the
/// `RepositoryStatusRevision`, `RepositoryStatusFile`, and `RepositoryStatusCount`
/// events into a typed result.
pub async fn status(api: &LoreApi, args: RepositoryStatusArgs) -> Result<RepositoryStatusResult> {
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

    let mut result = RepositoryStatusResult::default();

    for event in &stream.events {
        match event {
            LoreEvent::RepositoryStatusRevision(data) => {
                result.revision = Some(StatusRevision {
                    repository: format!("{}", data.repository),
                    branch: format!("{}", data.branch),
                    branch_name: data.branch_name.as_str().to_string(),
                    revision: hash_or_empty(&data.revision),
                    revision_number: data.revision_number,
                    revision_staged: hash_or_empty(&data.revision_staged),
                });
            }
            LoreEvent::RepositoryStatusFile(data) => {
                result.files.push(StatusFile {
                    path: data.path.as_str().to_string(),
                    size: data.size,
                    action: map_action(&data.action),
                    node_type: map_node_type(&data.r#type),
                    staged: data.flag_staged != 0,
                    conflict: data.flag_conflict != 0,
                    dirty: data.flag_dirty != 0,
                    from_path: data.from_path.as_str().to_string(),
                });
            }
            LoreEvent::RepositoryStatusCount(data) => {
                result.count = Some(StatusCount {
                    directories: data.directories,
                    files: data.files,
                });
            }
            _ => {}
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_args_defaults() {
        let json = r#"{}"#;
        let args: RepositoryStatusArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(!args.staged);
        assert!(!args.scan);
        assert!(args.paths.is_empty());
    }

    #[test]
    fn status_args_into_lore_conversion() {
        let args = RepositoryStatusArgs {
            staged: true,
            count: true,
            paths: vec!["a.txt".into()],
            ..Default::default()
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.staged, 1);
        assert_eq!(lore_args.count, 1);
        assert_eq!(lore_args.paths.len(), 1);
    }

    #[test]
    fn status_result_serializes() {
        let result = RepositoryStatusResult {
            revision: Some(StatusRevision {
                repository: "repo".into(),
                branch: "br".into(),
                branch_name: "main".into(),
                revision: "rev1".into(),
                revision_number: 1,
                revision_staged: String::new(),
            }),
            files: vec![StatusFile {
                path: "a.txt".into(),
                size: 11,
                action: StatusFileAction::Add,
                node_type: StatusNodeType::File,
                staged: true,
                conflict: false,
                dirty: false,
                from_path: String::new(),
            }],
            count: Some(StatusCount {
                directories: 0,
                files: 1,
            }),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("a.txt"));
        assert!(json.contains("main"));
        assert!(json.contains(r#""add""#));
    }
}
