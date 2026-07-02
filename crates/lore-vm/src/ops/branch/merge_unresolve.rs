//! `branch merge_unresolve` operation — binds `lore::branch::merge_unresolve`.
//!
//! Marks previously resolved merge paths as unresolved, restoring their
//! conflict state. Emits `BranchMergeUnresolveFile` for each affected path
//! and `BranchMergeUnresolveRevision` with the updated staged revision.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchMergeUnresolveArgs;
use lore::interface::LoreEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMergeUnresolveArgs {
    #[serde(default)]
    pub paths: Vec<String>,
}

impl BranchMergeUnresolveArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreBranchMergeUnresolveArgs {
        LoreBranchMergeUnresolveArgs {
            paths: crate::ops::paths::lore_path_args(repo_root, &self.paths),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMergeUnresolveResult {
    pub unresolved_paths: Vec<String>,
}

pub async fn merge_unresolve(
    api: &LoreApi,
    args: BranchMergeUnresolveArgs,
) -> Result<BranchMergeUnresolveResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::branch::merge_unresolve(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch merge_unresolve failed with status {status}"),
        )));
    }

    let unresolved_paths: Vec<String> = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::BranchMergeUnresolveFile(data) = event {
                Some(data.path.as_str().to_string())
            } else {
                None
            }
        })
        .collect();

    Ok(BranchMergeUnresolveResult { unresolved_paths })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_unresolve_args_serializes() {
        let args = BranchMergeUnresolveArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn merge_unresolve_args_deserializes_with_default() {
        let json = r#"{}"#;
        let args: BranchMergeUnresolveArgs =
            serde_json::from_str(json).expect("should deserialize");
        assert!(args.paths.is_empty());
    }

    #[test]
    fn merge_unresolve_args_into_lore_conversion() {
        let args = BranchMergeUnresolveArgs {
            paths: vec!["a.txt".into(), "b.txt".into()],
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.paths.as_slice().len(), 2);
    }

    #[test]
    fn merge_unresolve_result_serializes() {
        let result = BranchMergeUnresolveResult {
            unresolved_paths: vec!["conflict.rs".into()],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("conflict.rs"));
    }

    #[test]
    fn merge_unresolve_result_empty() {
        let result = BranchMergeUnresolveResult {
            unresolved_paths: vec![],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("[]"));
    }
}
