//! `repository info` operation — binds `lore::repository::info`.
//!
//! Retrieves metadata about a remote repository, such as its name, URL,
//! default branch, creator, and creation time.
//! Emits `LoreEvent::RepositoryData` on success.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::repository::LoreRepositoryInfoArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`info`].
///
/// Mirrors `LoreRepositoryInfoArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryInfoArgs {
    /// URL of the remote repository to query; empty string falls back to the
    /// repository configured in the current working directory.
    #[serde(default)]
    pub repository_url: String,
}

impl RepositoryInfoArgs {
    fn into_lore(self) -> LoreRepositoryInfoArgs {
        LoreRepositoryInfoArgs {
            repository_url: LoreString::from_str(&self.repository_url),
        }
    }
}

/// Result returned on successful repository info query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryInfoResult {
    /// Remote URL of the repository.
    pub remote_url: String,
    /// Repository identifier.
    pub id: String,
    /// Repository name.
    pub name: String,
    /// Repository description.
    pub description: String,
    /// Identifier of the default branch.
    pub default_branch: String,
    /// Name of the default branch.
    pub default_branch_name: String,
    /// User who created the repository.
    pub creator: String,
    /// Creation timestamp (Unix epoch seconds).
    pub created: u64,
}

/// Retrieve metadata about a remote repository.
///
/// Calls the upstream `lore::repository::info` in-process and collects
/// the `RepositoryData` event to return a typed result.
pub async fn info(api: &LoreApi, args: RepositoryInfoArgs) -> Result<RepositoryInfoResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::repository::info(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("repository info failed with status {status}"),
        )));
    }

    let data = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::RepositoryData(data) = event {
                Some(data.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            LoreError::Parse("repository info succeeded but no RepositoryData event emitted".into())
        })?;

    Ok(RepositoryInfoResult {
        remote_url: data.remote_url.as_str().to_string(),
        id: format!("{}", data.id),
        name: data.name.as_str().to_string(),
        description: data.description.as_str().to_string(),
        default_branch: format!("{}", data.default_branch),
        default_branch_name: data.default_branch_name.as_str().to_string(),
        creator: data.creator.as_str().to_string(),
        created: data.created,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_info_args_serializes() {
        let args = RepositoryInfoArgs {
            repository_url: "https://example.com/repo".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("https://example.com/repo"));
    }

    #[test]
    fn repository_info_args_deserializes_with_default() {
        let json = r#"{}"#;
        let args: RepositoryInfoArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.repository_url, "");
    }

    #[test]
    fn repository_info_args_into_lore_conversion() {
        let args = RepositoryInfoArgs {
            repository_url: "https://example.com/repo".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.repository_url.as_str(), "https://example.com/repo");
    }

    #[test]
    fn repository_info_result_serializes() {
        let result = RepositoryInfoResult {
            remote_url: "https://example.com".into(),
            id: "abc123".into(),
            name: "my-repo".into(),
            description: "A test repository".into(),
            default_branch: "branch-id".into(),
            default_branch_name: "main".into(),
            creator: "alice".into(),
            created: 1718000000,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("my-repo"));
        assert!(json.contains("https://example.com"));
        assert!(json.contains("alice"));
        assert!(json.contains("1718000000"));
        assert!(json.contains("A test repository"));
    }

    #[test]
    fn repository_info_result_empty_fields() {
        let result = RepositoryInfoResult {
            remote_url: String::new(),
            id: String::new(),
            name: String::new(),
            description: String::new(),
            default_branch: String::new(),
            default_branch_name: String::new(),
            creator: String::new(),
            created: 0,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""created":0"#));
    }

    #[test]
    fn repository_info_result_roundtrip() {
        let result = RepositoryInfoResult {
            remote_url: "https://example.com".into(),
            id: "id-1".into(),
            name: "repo-name".into(),
            description: "desc".into(),
            default_branch: "br-id".into(),
            default_branch_name: "main".into(),
            creator: "bob".into(),
            created: 99,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: RepositoryInfoResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.name, "repo-name");
        assert_eq!(deserialized.created, 99);
        assert_eq!(deserialized.creator, "bob");
    }
}
