//! `branch merge_resolve_mine` operation — binds `lore::branch::merge_resolve_mine`.
//!
//! Resolves merge conflicts for specified paths by accepting the local
//! ("mine") version. Emits `BranchMergeResolveFile` for each resolved path
//! and `BranchMergeResolveRevision` with the updated staged revision.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchMergeResolveMineArgs;
use lore::interface::{LoreArray, LoreEvent, LoreString};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMergeResolveMineArgs {
    #[serde(default)]
    pub paths: Vec<String>,
}

impl BranchMergeResolveMineArgs {
    fn into_lore(self, repo_root: &Path) -> LoreBranchMergeResolveMineArgs {
        LoreBranchMergeResolveMineArgs {
            paths: LoreArray::from_vec(
                self.paths.iter().map(|p| resolve_path(p, repo_root)).collect(),
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMergeResolveMineResult {
    pub resolved_paths: Vec<String>,
    pub revision: String,
}

pub async fn merge_resolve_mine(
    api: &LoreApi,
    args: BranchMergeResolveMineArgs,
) -> Result<BranchMergeResolveMineResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::branch::merge_resolve_mine(globals.build(), args.into_lore(&repo_root), callback)
            .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch merge_resolve_mine failed with status {status}"),
        )));
    }

    let mut resolved_paths = Vec::new();
    let mut revision = String::new();

    for event in &stream.events {
        match event {
            LoreEvent::BranchMergeResolveFile(data) => {
                resolved_paths.push(data.path.as_str().to_string());
            }
            LoreEvent::BranchMergeResolveRevision(data) => {
                revision = format!("{}", data.revision);
            }
            _ => {}
        }
    }

    Ok(BranchMergeResolveMineResult {
        resolved_paths,
        revision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_resolve_mine_args_serializes() {
        let args = BranchMergeResolveMineArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn merge_resolve_mine_args_deserializes_with_default() {
        let json = r#"{}"#;
        let args: BranchMergeResolveMineArgs =
            serde_json::from_str(json).expect("should deserialize");
        assert!(args.paths.is_empty());
    }

    #[test]
    fn merge_resolve_mine_args_into_lore_conversion() {
        let args = BranchMergeResolveMineArgs {
            paths: vec!["a.txt".into(), "b.txt".into()],
        };
        let repo_root = std::path::Path::new("/work/myrepo");
        let lore_args = args.into_lore(repo_root);
        assert_eq!(lore_args.paths.as_slice().len(), 2);
    }

    /// Regression: relative paths must be resolved against repo_root.
    #[test]
    fn merge_resolve_mine_args_resolves_relative_paths() {
        let args = BranchMergeResolveMineArgs {
            paths: vec!["src/main.rs".into()],
        };
        let repo_root = std::path::Path::new("/work/myrepo");
        let lore_args = args.into_lore(repo_root);
        assert_eq!(
            lore_args.paths.as_slice()[0].as_str(),
            "/work/myrepo/src/main.rs"
        );
    }

    #[test]
    fn merge_resolve_mine_result_serializes() {
        let result = BranchMergeResolveMineResult {
            resolved_paths: vec!["conflict.rs".into()],
            revision: "abc123def456".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("conflict.rs"));
        assert!(json.contains("abc123def456"));
    }

    #[test]
    fn merge_resolve_mine_result_empty() {
        let result = BranchMergeResolveMineResult {
            resolved_paths: vec![],
            revision: String::new(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("resolved_paths"));
        assert!(json.contains("revision"));
    }

    #[test]
    fn merge_resolve_mine_result_roundtrip() {
        let result = BranchMergeResolveMineResult {
            resolved_paths: vec!["a.rs".into(), "b.rs".into()],
            revision: "deadbeef".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let parsed: BranchMergeResolveMineResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(parsed.resolved_paths, result.resolved_paths);
        assert_eq!(parsed.revision, result.revision);
    }
}
