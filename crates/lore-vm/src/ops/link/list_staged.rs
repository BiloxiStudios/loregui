//! `link list_staged` operation — binds `lore::link::list_staged`.
//!
//! Lists links with staged changes in the current repository.
//! Calls [`lore::link::list_staged`] in-process (no CLI shelling) and collects
//! `LinkStagedEntry` events to return typed results.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use serde::{Deserialize, Serialize};

/// A single staged-link entry returned by `link list_staged`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedLinkEntry {
    /// Path of the link within the parent repository.
    pub path: String,
    /// Identifier of the repository the link points to.
    pub repository: String,
    /// Number of staged files inside the link.
    pub staged_file_count: u64,
}

/// Result of a successful `link list_staged` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkListStagedResult {
    /// Number of links with staged changes.
    pub link_count: u32,
    /// Details of each link with staged changes.
    pub links: Vec<StagedLinkEntry>,
}

/// List all links with staged changes in the current repository.
///
/// Calls upstream `lore::link::list_staged` in-process, collects the
/// `LinkStagedEntry` events emitted for each link with staged changes,
/// and returns a typed result.
pub async fn list_staged(api: &LoreApi) -> Result<LinkListStagedResult> {
    let (callback, rx) = collect_events();

    let status = lore::link::list_staged(api.globals().build(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("link list_staged failed with status {status}"),
        )));
    }

    let mut links = Vec::new();
    for event in &stream.events {
        if let LoreEvent::LinkStagedEntry(data) = event {
            links.push(StagedLinkEntry {
                path: data.path.as_str().to_string(),
                repository: format!("{}", data.repository),
                staged_file_count: data.staged_file_count,
            });
        }
    }

    Ok(LinkListStagedResult {
        link_count: links.len() as u32,
        links,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_link_entry_serializes() {
        let entry = StagedLinkEntry {
            path: "deps/characters".into(),
            repository: "city-of-brains".into(),
            staged_file_count: 5,
        };

        assert_eq!(entry.path, "deps/characters");
        assert_eq!(entry.repository, "city-of-brains");
        assert_eq!(entry.staged_file_count, 5);

        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("\"path\":\"deps/characters\""));
        assert!(json.contains("\"repository\":\"city-of-brains\""));
        assert!(json.contains("\"staged_file_count\":5"));
    }

    #[test]
    fn link_list_staged_result_serialization() {
        let result = LinkListStagedResult {
            link_count: 2,
            links: vec![
                StagedLinkEntry {
                    path: "deps/characters".into(),
                    repository: "city-of-brains".into(),
                    staged_file_count: 5,
                },
                StagedLinkEntry {
                    path: "deps/world".into(),
                    repository: "world-builder".into(),
                    staged_file_count: 3,
                },
            ],
        };

        assert_eq!(result.link_count, 2);
        assert_eq!(result.links.len(), 2);

        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: LinkListStagedResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.link_count, 2);
        assert_eq!(deserialized.links[0].path, "deps/characters");
        assert_eq!(deserialized.links[1].repository, "world-builder");
    }

    #[test]
    fn empty_link_list_staged_result() {
        let result = LinkListStagedResult {
            link_count: 0,
            links: vec![],
        };

        assert_eq!(result.link_count, 0);
        assert!(result.links.is_empty());

        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("\"link_count\":0"));
        assert!(json.contains("\"links\":[]"));
    }

    #[test]
    fn staged_link_entry_with_zero_files() {
        let entry = StagedLinkEntry {
            path: "deps/empty".into(),
            repository: "empty-repo".into(),
            staged_file_count: 0,
        };

        assert_eq!(entry.staged_file_count, 0);

        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("\"staged_file_count\":0"));
    }
}
