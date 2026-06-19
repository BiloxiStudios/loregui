//! `repository dump` operation — binds `lore::repository::dump`.
//!
//! Dumps the internal state tree of the repository for diagnostic purposes.
//! Emits `RepositoryDumpBegin`, `RepositoryStateDump` (revision summary),
//! `RepositoryStateDumpNode` (per-node detail), and `RepositoryDumpEnd`.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::repository::LoreRepositoryDumpArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`dump`].
///
/// Mirrors `LoreRepositoryDumpArgs` from the upstream `lore` crate but uses
/// plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepositoryDumpArgs {
    /// Revision to dump; empty string uses the current revision.
    #[serde(default)]
    pub revision: String,
    /// Repository-relative path to start from; empty dumps the root.
    #[serde(default)]
    pub path: String,
    /// Maximum tree traversal depth (0 = unlimited).
    #[serde(default)]
    pub max_depth: usize,
}

impl RepositoryDumpArgs {
    fn into_lore(self) -> LoreRepositoryDumpArgs {
        LoreRepositoryDumpArgs {
            revision: LoreString::from_str(&self.revision),
            path: LoreString::from_str(&self.path),
            max_depth: self.max_depth,
        }
    }
}

/// Summary of the dumped state (from `RepositoryStateDump` event).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpStateSummary {
    /// Sequence number of the revision.
    pub revision_number: u64,
    /// Hash of the revision.
    pub revision: String,
    /// Hash of the state's node tree.
    pub tree_hash: String,
    /// Size of the node tree in bytes.
    pub tree_size: u64,
}

/// A single node in the dumped state tree (from `RepositoryStateDumpNode` event).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpNode {
    /// Name of the node.
    pub name: String,
    /// Identifier of the node.
    pub id: u32,
    /// Identifier of the parent node.
    pub parent: u32,
    /// Identifier of the next sibling node.
    pub sibling: u32,
    /// File mode of the node.
    pub mode: u16,
    /// Size of the node's content in bytes.
    pub size: u64,
    /// Node flags.
    pub flags: u16,
    /// Type-specific detail for the node.
    pub type_data: String,
}

/// Result returned on a successful repository dump.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepositoryDumpResult {
    /// Repository identifier from the DumpBegin event.
    pub repository: String,
    /// Revision hash from the DumpBegin event.
    pub begin_revision: String,
    /// State summary, when emitted.
    pub state: Option<DumpStateSummary>,
    /// One entry per node in the state tree.
    pub nodes: Vec<DumpNode>,
    /// Diagnostic log messages emitted during the dump.
    pub log_messages: Vec<String>,
}

/// Dump the internal state tree of the repository.
///
/// Calls the upstream `lore::repository::dump` in-process and collects
/// `RepositoryDumpBegin`, `RepositoryStateDump`, `RepositoryStateDumpNode`,
/// and `RepositoryDumpEnd` events into a typed result.
pub async fn dump(api: &LoreApi, args: RepositoryDumpArgs) -> Result<RepositoryDumpResult> {
    let (callback, rx) = collect_events();

    let status = lore::repository::dump(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("repository dump failed with status {status}"),
        )));
    }

    let mut result = RepositoryDumpResult::default();

    for event in &stream.events {
        match event {
            LoreEvent::RepositoryDumpBegin(data) => {
                result.repository = format!("{}", data.repository);
                result.begin_revision = format!("{}", data.revision);
            }
            LoreEvent::RepositoryStateDump(data) => {
                result.state = Some(DumpStateSummary {
                    revision_number: data.revision_number,
                    revision: format!("{}", data.revision),
                    tree_hash: format!("{}", data.tree_hash),
                    tree_size: data.tree_size,
                });
            }
            LoreEvent::RepositoryStateDumpNode(data) => {
                result.nodes.push(DumpNode {
                    name: data.name.as_str().to_string(),
                    id: data.id,
                    parent: data.parent,
                    sibling: data.sibling,
                    mode: data.mode,
                    size: data.size,
                    flags: data.flags,
                    type_data: data.type_data.as_str().to_string(),
                });
            }
            LoreEvent::Log(data) => {
                result.log_messages.push(data.message.as_str().to_string());
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
    fn dump_args_defaults() {
        let json = r#"{}"#;
        let args: RepositoryDumpArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.revision, "");
        assert_eq!(args.path, "");
        assert_eq!(args.max_depth, 0);
    }

    #[test]
    fn dump_args_into_lore_conversion() {
        let args = RepositoryDumpArgs {
            revision: "abc123".into(),
            path: "subdir".into(),
            max_depth: 5,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.revision.as_str(), "abc123");
        assert_eq!(lore_args.path.as_str(), "subdir");
        assert_eq!(lore_args.max_depth, 5);
    }

    #[test]
    fn result_serialises() {
        let result = RepositoryDumpResult {
            repository: "repo-id".into(),
            begin_revision: "rev-hash".into(),
            state: Some(DumpStateSummary {
                revision_number: 1,
                revision: "rev1".into(),
                tree_hash: "tree1".into(),
                tree_size: 1024,
            }),
            nodes: vec![DumpNode {
                name: "root".into(),
                id: 0,
                parent: 0,
                sibling: 0,
                mode: 0o755,
                size: 0,
                flags: 0,
                type_data: String::new(),
            }],
            log_messages: vec!["dump complete".into()],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("repo-id"));
        assert!(json.contains("root"));
        assert!(json.contains("dump complete"));
    }

    #[test]
    fn empty_result() {
        let result = RepositoryDumpResult::default();
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("\"nodes\":[]"));
    }

    #[test]
    fn result_roundtrip() {
        let result = RepositoryDumpResult {
            repository: "r".into(),
            begin_revision: "b".into(),
            state: None,
            nodes: vec![],
            log_messages: vec!["hello".into()],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        let deserialized: RepositoryDumpResult = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(deserialized.repository, "r");
        assert_eq!(deserialized.log_messages.len(), 1);
    }
}
