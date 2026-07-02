//! `branch diff` operation — binds `lore::branch::diff`.
//!
//! Computes the diff between two branches, reporting changed and conflicting
//! files.  Emits `BranchDiffChange` for each changed file and
//! `BranchDiffConflict` for each file that conflicts between the two branches.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchDiffArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`diff`].
///
/// Mirrors `LoreBranchDiffArgs` from the upstream `lore` crate but uses plain
/// `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchDiffArgs {
    /// Source branch name.
    pub source: String,
    /// Target branch name.
    pub target: String,
    /// Optional path to limit the diff to; empty means all files.
    #[serde(default)]
    pub path: String,
    /// Attempt to auto-resolve conflicts.
    #[serde(default)]
    pub auto_resolve: bool,
}

impl BranchDiffArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreBranchDiffArgs {
        LoreBranchDiffArgs {
            source: LoreString::from_str(&self.source),
            target: LoreString::from_str(&self.target),
            path: crate::ops::paths::lore_path_arg(repo_root, &self.path),
            auto_resolve: u8::from(self.auto_resolve),
        }
    }
}

/// The action applied to a node in the branch diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BranchDiffAction {
    Keep,
    Add,
    Delete,
    Move,
    Copy,
}

/// A single changed node in the branch diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchDiffChange {
    /// Path of the changed node.
    pub path: String,
    /// Action applied to the node.
    pub action: BranchDiffAction,
    /// Whether the change was auto-merged.
    pub automerged: bool,
}

/// A single conflicting pair in the branch diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchDiffConflict {
    /// The change on the source side.
    pub source_change: BranchDiffChange,
    /// The change on the target side.
    pub target_change: BranchDiffChange,
}

/// Result returned on a successful branch diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchDiffResult {
    /// Files that changed between the two branches.
    pub changes: Vec<BranchDiffChange>,
    /// Files that conflict between the two branches.
    pub conflicts: Vec<BranchDiffConflict>,
}

fn map_action(action: lore::interface::LoreFileAction) -> BranchDiffAction {
    match action {
        lore::interface::LoreFileAction::Keep => BranchDiffAction::Keep,
        lore::interface::LoreFileAction::Add => BranchDiffAction::Add,
        lore::interface::LoreFileAction::Delete => BranchDiffAction::Delete,
        lore::interface::LoreFileAction::Move => BranchDiffAction::Move,
        lore::interface::LoreFileAction::Copy => BranchDiffAction::Copy,
    }
}

/// Compute the diff between two branches.
///
/// Calls the upstream `lore::branch::diff` in-process and collects
/// `BranchDiffChange` and `BranchDiffConflict` events into a typed result.
pub async fn diff(api: &LoreApi, args: BranchDiffArgs) -> Result<BranchDiffResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status = lore::branch::diff(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch diff failed with status {status}"),
        )));
    }

    let mut changes = Vec::new();
    let mut conflicts = Vec::new();

    for event in &stream.events {
        match event {
            LoreEvent::BranchDiffChange(data) => {
                changes.push(BranchDiffChange {
                    path: data.change.path.as_str().to_string(),
                    action: map_action(data.change.action),
                    automerged: data.change.automerged != 0,
                });
            }
            LoreEvent::BranchDiffConflict(data) => {
                conflicts.push(BranchDiffConflict {
                    source_change: BranchDiffChange {
                        path: data.source_change.path.as_str().to_string(),
                        action: map_action(data.source_change.action),
                        automerged: data.source_change.automerged != 0,
                    },
                    target_change: BranchDiffChange {
                        path: data.target_change.path.as_str().to_string(),
                        action: map_action(data.target_change.action),
                        automerged: data.target_change.automerged != 0,
                    },
                });
            }
            _ => {}
        }
    }

    Ok(BranchDiffResult { changes, conflicts })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_args_serializes() {
        let args = BranchDiffArgs {
            source: "feature/x".into(),
            target: "main".into(),
            path: "src/".into(),
            auto_resolve: true,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("feature/x"));
        assert!(json.contains("main"));
        assert!(json.contains("src/"));
        assert!(json.contains(r#""auto_resolve":true"#));
    }

    #[test]
    fn diff_args_deserializes_with_defaults() {
        let json = r#"{"source":"a","target":"b"}"#;
        let args: BranchDiffArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.source, "a");
        assert_eq!(args.target, "b");
        assert_eq!(args.path, "");
        assert!(!args.auto_resolve);
    }

    #[test]
    fn diff_args_into_lore_conversion() {
        let args = BranchDiffArgs {
            source: "dev".into(),
            target: "main".into(),
            path: "assets/".into(),
            auto_resolve: true,
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.source.as_str(), "dev");
        assert_eq!(lore_args.target.as_str(), "main");
        assert_eq!(lore_args.path.as_str(), "/repo/assets/");
        assert_eq!(lore_args.auto_resolve, 1);
    }

    #[test]
    fn diff_args_auto_resolve_false() {
        let args = BranchDiffArgs {
            source: "a".into(),
            target: "b".into(),
            path: String::new(),
            auto_resolve: false,
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.auto_resolve, 0);
    }

    #[test]
    fn diff_args_empty_path_preserved() {
        let args = BranchDiffArgs {
            source: "a".into(),
            target: "b".into(),
            path: String::new(),
            auto_resolve: false,
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo/root"));
        assert_eq!(
            lore_args.path.as_str(),
            "",
            "empty path must stay empty (no-filter sentinel), not become repo_root"
        );
    }

    #[test]
    fn diff_args_absolute_path_preserved() {
        let args = BranchDiffArgs {
            source: "a".into(),
            target: "b".into(),
            path: "/abs/path.txt".into(),
            auto_resolve: false,
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo/root"));
        assert_eq!(lore_args.path.as_str(), "/abs/path.txt");
    }

    #[test]
    fn diff_args_relative_path_joined_to_root() {
        let args = BranchDiffArgs {
            source: "a".into(),
            target: "b".into(),
            path: "src/lib.rs".into(),
            auto_resolve: false,
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo/root"));
        assert_eq!(lore_args.path.as_str(), "/repo/root/src/lib.rs");
    }

    #[test]
    fn branch_diff_action_serializes_all_variants() {
        assert_eq!(
            serde_json::to_string(&BranchDiffAction::Keep).unwrap(),
            r#""keep""#
        );
        assert_eq!(
            serde_json::to_string(&BranchDiffAction::Add).unwrap(),
            r#""add""#
        );
        assert_eq!(
            serde_json::to_string(&BranchDiffAction::Delete).unwrap(),
            r#""delete""#
        );
        assert_eq!(
            serde_json::to_string(&BranchDiffAction::Move).unwrap(),
            r#""move""#
        );
        assert_eq!(
            serde_json::to_string(&BranchDiffAction::Copy).unwrap(),
            r#""copy""#
        );
    }

    #[test]
    fn branch_diff_action_roundtrips() {
        for action in [
            BranchDiffAction::Keep,
            BranchDiffAction::Add,
            BranchDiffAction::Delete,
            BranchDiffAction::Move,
            BranchDiffAction::Copy,
        ] {
            let json = serde_json::to_string(&action).unwrap();
            let parsed: BranchDiffAction = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, action);
        }
    }

    #[test]
    fn branch_diff_change_serializes() {
        let change = BranchDiffChange {
            path: "src/main.rs".into(),
            action: BranchDiffAction::Add,
            automerged: false,
        };
        let json = serde_json::to_string(&change).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains(r#""add""#));
        assert!(json.contains(r#""automerged":false"#));
    }

    #[test]
    fn branch_diff_conflict_serializes() {
        let conflict = BranchDiffConflict {
            source_change: BranchDiffChange {
                path: "README.md".into(),
                action: BranchDiffAction::Delete,
                automerged: false,
            },
            target_change: BranchDiffChange {
                path: "README.md".into(),
                action: BranchDiffAction::Keep,
                automerged: false,
            },
        };
        let json = serde_json::to_string(&conflict).expect("should serialize");
        assert!(json.contains("source_change"));
        assert!(json.contains("target_change"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn branch_diff_result_serializes() {
        let result = BranchDiffResult {
            changes: vec![BranchDiffChange {
                path: "a.txt".into(),
                action: BranchDiffAction::Add,
                automerged: true,
            }],
            conflicts: vec![BranchDiffConflict {
                source_change: BranchDiffChange {
                    path: "b.txt".into(),
                    action: BranchDiffAction::Delete,
                    automerged: false,
                },
                target_change: BranchDiffChange {
                    path: "b.txt".into(),
                    action: BranchDiffAction::Keep,
                    automerged: false,
                },
            }],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("a.txt"));
        assert!(json.contains("b.txt"));
        assert!(json.contains("changes"));
        assert!(json.contains("conflicts"));
    }

    #[test]
    fn branch_diff_result_empty() {
        let result = BranchDiffResult {
            changes: vec![],
            conflicts: vec![],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""changes":[]"#));
        assert!(json.contains(r#""conflicts":[]"#));
    }
}
