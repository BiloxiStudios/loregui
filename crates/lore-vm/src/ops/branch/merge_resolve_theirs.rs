//! `branch merge_resolve_theirs` operation — binds `lore::branch::merge_resolve_theirs`.
//!
//! Resolves merge conflicts for specified paths by accepting the incoming
//! ("theirs") version. Emits `BranchMergeResolveFile` for each resolved path
//! and `BranchMergeResolveRevision` with the updated staged revision.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchMergeResolveTheirsArgs;
use lore::interface::{LoreArray, LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMergeResolveTheirsArgs {
    #[serde(default)]
    pub paths: Vec<String>,
}

impl BranchMergeResolveTheirsArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreBranchMergeResolveTheirsArgs {
        LoreBranchMergeResolveTheirsArgs {
            paths: LoreArray::from_vec(
                self.paths
                    .iter()
                    .map(|p| {
                        let path = std::path::Path::new(p);
                        if path.is_absolute() {
                            LoreString::from_str(p)
                        } else {
                            LoreString::from_path(repo_root.join(path))
                        }
                    })
                    .collect(),
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMergeResolveTheirsResult {
    pub resolved_paths: Vec<String>,
    pub revision: String,
}

pub async fn merge_resolve_theirs(
    api: &LoreApi,
    args: BranchMergeResolveTheirsArgs,
) -> Result<BranchMergeResolveTheirsResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::branch::merge_resolve_theirs(globals.build(), args.into_lore(&repo_root), callback)
            .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch merge_resolve_theirs failed with status {status}"),
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

    Ok(BranchMergeResolveTheirsResult {
        resolved_paths,
        revision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_resolve_theirs_args_serializes() {
        let args = BranchMergeResolveTheirsArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn merge_resolve_theirs_args_deserializes_with_default() {
        let json = r#"{}"#;
        let args: BranchMergeResolveTheirsArgs =
            serde_json::from_str(json).expect("should deserialize");
        assert!(args.paths.is_empty());
    }

    #[test]
    fn merge_resolve_theirs_args_into_lore_conversion() {
        let args = BranchMergeResolveTheirsArgs {
            paths: vec!["a.txt".into(), "b.txt".into()],
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.paths.as_slice().len(), 2);
    }

    #[test]
    fn merge_resolve_theirs_result_serializes() {
        let result = BranchMergeResolveTheirsResult {
            resolved_paths: vec!["conflict.rs".into()],
            revision: "abc123def456".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("conflict.rs"));
        assert!(json.contains("abc123def456"));
    }

    #[test]
    fn merge_resolve_theirs_result_empty() {
        let result = BranchMergeResolveTheirsResult {
            resolved_paths: vec![],
            revision: String::new(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("resolved_paths"));
        assert!(json.contains("revision"));
    }

    #[test]
    fn merge_resolve_theirs_result_roundtrip() {
        let result = BranchMergeResolveTheirsResult {
            resolved_paths: vec!["a.rs".into(), "b.rs".into()],
            revision: "deadbeef".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let parsed: BranchMergeResolveTheirsResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(parsed.resolved_paths, result.resolved_paths);
        assert_eq!(parsed.revision, result.revision);
    }
}
