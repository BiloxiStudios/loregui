//! `link list` operation — binds `lore::link::list`.
//!
//! Lists all linked repositories registered in the current repository.
//! Calls [`lore::link::list`] in-process (no CLI shelling) and collects
//! `LoreEvent::LinkEntry` events to return typed results.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::link::LoreLinkListArgs;
use serde::{Deserialize, Serialize};

/// A single linked-repository entry returned by `link list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkEntry {
    /// Path of the link within the repository.
    pub link_path: String,
    /// Target link identifier.
    pub link: String,
    /// Source node in the repository.
    pub source_node: String,
    /// Link node identifier.
    pub link_node: String,
}

/// Result of a successful `link list` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkListResult {
    /// Number of linked repositories found.
    pub link_count: u32,
    /// Details of each linked repository.
    pub links: Vec<LinkEntry>,
}

/// List all linked repositories in the current repository.
///
/// Calls upstream `lore::link::list` in-process, collects the `LinkEntry`
/// events emitted for each linked repo, and returns a typed result.
pub async fn list(api: &LoreApi) -> Result<LinkListResult> {
    let args = LoreLinkListArgs {};
    let (callback, rx) = collect_events();

    let status = lore::link::list(api.globals().build(), args, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("link list failed with status {status}"),
        )));
    }

    let mut links = Vec::new();
    for event in &stream.events {
        if let LoreEvent::LinkEntry(data) = event {
            links.push(LinkEntry {
                link_path: data.link_path.as_str().to_string(),
                link: format!("{}", data.link),
                source_node: format!("{}", data.source_node),
                link_node: format!("{}", data.link_node),
            });
        }
    }

    Ok(LinkListResult {
        link_count: links.len() as u32,
        links,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = LinkListResult {
            link_count: 1,
            links: vec![LinkEntry {
                link_path: "deps/characters".into(),
                link: "city-of-brains".into(),
                source_node: "root".into(),
                link_node: "nodes/1".into(),
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"link_count\":1"));
        assert!(json.contains("\"link_path\":\"deps/characters\""));
    }

    #[test]
    fn empty_list_result() {
        let result = LinkListResult {
            link_count: 0,
            links: vec![],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"link_count\":0"));
        assert!(json.contains("\"links\":[]"));
    }

    #[test]
    fn link_entry_args_is_empty() {
        // LoreLinkListArgs has no fields; verify it can be constructed.
        let _args = LoreLinkListArgs {};
    }
}
