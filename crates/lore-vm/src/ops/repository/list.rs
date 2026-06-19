//! `repository list` operation — binds `lore::repository::list`.
//!
//! Lists all repositories available at a given remote URL.
//! Emits `LoreEvent::RepositoryListEntry` for each repository found.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::repository::LoreRepositoryListArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`list`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListArgs {
    /// Remote URL to list repositories from.
    pub url: String,
}

impl ListArgs {
    fn into_lore(self) -> LoreRepositoryListArgs {
        LoreRepositoryListArgs {
            url: LoreString::from_str(&self.url),
        }
    }
}

/// A single repository entry returned by [`list`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryEntry {
    /// Repository identifier (hex string).
    pub id: String,
    /// Repository name.
    pub name: String,
}

/// Result returned on successful repository listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResult {
    /// The remote URL that was queried.
    pub url: String,
    /// Discovered repositories.
    pub entries: Vec<RepositoryEntry>,
}

/// List all repositories available at the given remote URL.
pub async fn list(api: &LoreApi, args: ListArgs) -> Result<ListResult> {
    let url = args.url.clone();
    let (callback, rx) = collect_events();

    let status = lore::repository::list(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("repository list failed with status {status}"),
        )));
    }

    let mut entries = Vec::new();
    for event in &stream.events {
        if let LoreEvent::RepositoryListEntry(data) = event {
            entries.push(RepositoryEntry {
                id: format!("{}", data.id),
                name: data.name.as_str().to_string(),
            });
        }
    }

    Ok(ListResult { url, entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_args_serializes() {
        let args = ListArgs {
            url: "lore://example.com/myrepo".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("example.com"));
    }

    #[test]
    fn list_args_deserializes() {
        let json = r#"{"url":"lore://host/repo"}"#;
        let args: ListArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.url, "lore://host/repo");
    }

    #[test]
    fn list_args_into_lore_conversion() {
        let args = ListArgs {
            url: "lore://remote.example/project".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.url.as_str(), "lore://remote.example/project");
    }

    #[test]
    fn list_result_serializes() {
        let result = ListResult {
            url: "lore://example.com".into(),
            entries: vec![
                RepositoryEntry {
                    id: "repo-1".into(),
                    name: "MyProject".into(),
                },
                RepositoryEntry {
                    id: "repo-2".into(),
                    name: "AnotherRepo".into(),
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("MyProject"));
        assert!(json.contains("AnotherRepo"));
    }

    #[test]
    fn list_result_empty_entries() {
        let result = ListResult {
            url: "lore://empty.example".into(),
            entries: vec![],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""entries":[]"#));
    }

    #[test]
    fn repository_entry_roundtrip() {
        let entry = RepositoryEntry {
            id: "abc-123".into(),
            name: "TestRepo".into(),
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        let deserialized: RepositoryEntry =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.id, entry.id);
        assert_eq!(deserialized.name, entry.name);
    }
}
