//! `branch merge_restart` operation — binds `lore::branch::merge_restart`.
//!
//! Re-applies merge conflict resolution for specified paths, re-materializing
//! their working copies. Emits `BranchMergeConflictFile` for each path still
//! in conflict and `RevisionSyncFile` for each file re-materialized.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchMergeRestartArgs;
use lore::interface::{LoreArray, LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMergeRestartArgs {
    #[serde(default)]
    pub paths: Vec<String>,
}

impl BranchMergeRestartArgs {
    fn into_lore(self) -> LoreBranchMergeRestartArgs {
        LoreBranchMergeRestartArgs {
            paths: LoreArray::from_vec(
                self.paths.iter().map(|p| LoreString::from_str(p)).collect(),
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRestartSyncedFile {
    pub path: String,
    pub size: u64,
    pub action: String,
    pub is_file: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMergeRestartResult {
    pub conflict_files: Vec<String>,
    pub synced_files: Vec<MergeRestartSyncedFile>,
}

pub async fn merge_restart(
    api: &LoreApi,
    args: BranchMergeRestartArgs,
) -> Result<BranchMergeRestartResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::branch::merge_restart(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch merge_restart failed with status {status}"),
        )));
    }

    let mut conflict_files = Vec::new();
    let mut synced_files = Vec::new();

    for event in &stream.events {
        match event {
            LoreEvent::BranchMergeConflictFile(data) => {
                conflict_files.push(data.path.as_str().to_string());
            }
            LoreEvent::RevisionSyncFile(data) => {
                synced_files.push(MergeRestartSyncedFile {
                    path: data.path.as_str().to_string(),
                    size: data.size,
                    action: format!("{:?}", data.action),
                    is_file: data.flag_file != 0,
                });
            }
            _ => {}
        }
    }

    Ok(BranchMergeRestartResult {
        conflict_files,
        synced_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_restart_args_serializes() {
        let args = BranchMergeRestartArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn merge_restart_args_deserializes_with_default() {
        let json = r#"{}"#;
        let args: BranchMergeRestartArgs =
            serde_json::from_str(json).expect("should deserialize");
        assert!(args.paths.is_empty());
    }

    #[test]
    fn merge_restart_args_into_lore_conversion() {
        let args = BranchMergeRestartArgs {
            paths: vec!["a.txt".into(), "b.txt".into()],
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.paths.as_slice().len(), 2);
    }

    #[test]
    fn merge_restart_result_serializes() {
        let result = BranchMergeRestartResult {
            conflict_files: vec!["conflict.rs".into()],
            synced_files: vec![MergeRestartSyncedFile {
                path: "synced.rs".into(),
                size: 1024,
                action: "add".into(),
                is_file: true,
            }],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("conflict.rs"));
        assert!(json.contains("synced.rs"));
        assert!(json.contains("1024"));
    }

    #[test]
    fn merge_restart_result_empty() {
        let result = BranchMergeRestartResult {
            conflict_files: vec![],
            synced_files: vec![],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("conflict_files"));
        assert!(json.contains("synced_files"));
    }

    #[test]
    fn merge_restart_synced_file_roundtrip() {
        let file = MergeRestartSyncedFile {
            path: "assets/image.png".into(),
            size: 4096,
            action: "modify".into(),
            is_file: true,
        };
        let json = serde_json::to_string(&file).expect("should serialize");
        let parsed: MergeRestartSyncedFile =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(parsed.path, "assets/image.png");
        assert_eq!(parsed.size, 4096);
        assert!(parsed.is_file);
    }
}
