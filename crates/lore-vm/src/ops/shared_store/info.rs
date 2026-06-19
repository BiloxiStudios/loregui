//! `shared_store info` operation — binds `lore::shared_store::info`.
//!
//! Returns information about the configured default shared stores including
//! their remote URLs, filesystem paths, existence status, and whether they're
//! used automatically. Emits `LoreEvent::SharedStoreInfo` on success.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::shared_store::LoreSharedStoreInfoArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`info`].
///
/// Mirrors `LoreSharedStoreInfoArgs` from the upstream `lore` crate
/// (empty struct — no parameters needed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedStoreInfoArgs;

impl SharedStoreInfoArgs {
    fn into_lore(self) -> LoreSharedStoreInfoArgs {
        LoreSharedStoreInfoArgs {}
    }
}

/// Information about a single configured shared store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedStoreEntry {
    /// Remote URL backing the store.
    pub remote_url: String,
    /// Filesystem path of the store.
    pub path: String,
    /// Whether the store exists on disk.
    pub exists: bool,
}

/// Result returned on successful shared store info query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedStoreInfoResult {
    /// Whether shared stores are used automatically for the repository.
    pub use_automatically: bool,
    /// List of configured shared stores.
    pub stores: Vec<SharedStoreEntry>,
}

/// Retrieve information about the configured default shared stores.
///
/// Calls the upstream `lore::shared_store::info` in-process and collects
/// the `SharedStoreInfo` event to return a typed result.
pub async fn info(api: &LoreApi, _args: SharedStoreInfoArgs) -> Result<SharedStoreInfoResult> {
    let (callback, rx) = collect_events();

    let status = lore::shared_store::info(api.globals().build(), LoreSharedStoreInfoArgs {}, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("shared_store info failed with status {status}"),
        )));
    }

    let data = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::SharedStoreInfo(data) = event {
                Some(data.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            LoreError::Parse("shared_store info succeeded but no SharedStoreInfo event emitted".into())
        })?;

    let stores: Vec<SharedStoreEntry> = data
        .remote_urls
        .as_slice()
        .iter()
        .zip(data.paths.as_slice().iter())
        .zip(data.exists.as_slice().iter())
        .map(|((remote_url, path), exists)| SharedStoreEntry {
            remote_url: remote_url.as_str().to_string(),
            path: path.as_str().to_string(),
            exists: *exists != 0,
        })
        .collect();

    Ok(SharedStoreInfoResult {
        use_automatically: data.use_automatically != 0,
        stores,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_store_info_args_serializes() {
        let args = SharedStoreInfoArgs;
        let json = serde_json::to_string(&args).expect("should serialize");
        assert_eq!(json, "{}");
    }

    #[test]
    fn shared_store_info_args_deserializes() {
        let json = r#"{}"#;
        let args: SharedStoreInfoArgs = serde_json::from_str(json).expect("should deserialize");
        // Just verify it doesn't panic — struct is empty
        drop(args);
    }

    #[test]
    fn shared_store_info_result_serializes() {
        let result = SharedStoreInfoResult {
            use_automatically: true,
            stores: vec![
                SharedStoreEntry {
                    remote_url: "https://example.com/store".into(),
                    path: "/path/to/store".into(),
                    exists: true,
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""use_automatically":true"#));
        assert!(json.contains("https://example.com/store"));
        assert!(json.contains("/path/to/store"));
    }

    #[test]
    fn shared_store_info_result_empty_stores() {
        let result = SharedStoreInfoResult {
            use_automatically: false,
            stores: vec![],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""use_automatically":false"#));
        assert!(json.contains(r#""stores":[]"#));
    }

    #[test]
    fn shared_store_info_result_multiple_stores() {
        let result = SharedStoreInfoResult {
            use_automatically: true,
            stores: vec![
                SharedStoreEntry {
                    remote_url: "https://one.example.com".into(),
                    path: "/one/path".into(),
                    exists: true,
                },
                SharedStoreEntry {
                    remote_url: "https://two.example.com".into(),
                    path: "/two/path".into(),
                    exists: false,
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("https://one.example.com"));
        assert!(json.contains("https://two.example.com"));
    }
}
