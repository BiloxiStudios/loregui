//! `repository store_immutable_query` operation — binds `lore::repository::store_immutable_query`.
//!
//! Queries the local immutable store for fragments matching a given address.
//! Each matching fragment is delivered as a `RepositoryStoreImmutableQuery` event
//! containing its address, status, payload/content sizes, and flags.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::repository::LoreRepositoryStoreImmutableQueryArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`store_immutable_query`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreImmutableQueryArgs {
    /// Fragment address to query.
    pub address: String,
    /// When true, recurse into and query subfragments.
    #[serde(default)]
    pub recurse: bool,
}

impl StoreImmutableQueryArgs {
    fn into_lore(self) -> LoreRepositoryStoreImmutableQueryArgs {
        LoreRepositoryStoreImmutableQueryArgs {
            address: LoreString::from_str(&self.address),
            recurse: u8::from(self.recurse),
        }
    }
}

/// A single fragment entry found in the immutable store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreImmutableQueryEntry {
    /// Fragment address.
    pub address: String,
    /// Whether the result is from a remote store.
    pub remote: bool,
    /// Match status: 0 = exact, 1 = hash in repo, 2 = hash in other repo, 3 = not found.
    pub status: u32,
    /// Human-readable status label.
    pub status_label: String,
    /// Whether payload data is present in the store.
    pub payload: bool,
    /// Whether this fragment was a subfragment of the original query.
    pub subfragment: bool,
    /// Internal flags.
    pub flags: u32,
    /// Payload size in bytes.
    pub payload_size: u32,
    /// Content size in bytes.
    pub content_size: u64,
}

/// Result of a successful `store_immutable_query` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreImmutableQueryResult {
    /// Fragment entries found in the immutable store.
    pub entries: Vec<StoreImmutableQueryEntry>,
}

fn status_label(status: u32) -> &'static str {
    match status {
        0 => "stored",
        1 => "hash_exists",
        2 => "hash_exists_other_repo",
        3 => "not_found",
        _ => "unknown",
    }
}

/// Query the local immutable store for fragments matching an address.
///
/// Calls upstream `lore::repository::store_immutable_query` in-process,
/// collects `RepositoryStoreImmutableQuery` events, and returns typed entries.
pub async fn store_immutable_query(
    api: &LoreApi,
    args: StoreImmutableQueryArgs,
) -> Result<StoreImmutableQueryResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::repository::store_immutable_query(api.globals().build(), args.into_lore(), callback)
            .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("repository store_immutable_query failed with status {status}"),
        )));
    }

    let mut entries = Vec::new();

    for event in &stream.events {
        if let LoreEvent::RepositoryStoreImmutableQuery(data) = event {
            entries.push(StoreImmutableQueryEntry {
                address: format!("{}", data.address),
                remote: data.remote != 0,
                status: data.status,
                status_label: status_label(data.status).into(),
                payload: data.payload != 0,
                subfragment: data.subfragment != 0,
                flags: data.flags,
                payload_size: data.payload_size,
                content_size: data.content_size,
            });
        }
    }

    Ok(StoreImmutableQueryResult { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes() {
        let args = StoreImmutableQueryArgs {
            address: "abc123".into(),
            recurse: true,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("abc123"));
        assert!(json.contains("true"));
    }

    #[test]
    fn args_deserializes_with_default() {
        let args: StoreImmutableQueryArgs =
            serde_json::from_str(r#"{"address":"test"}"#).expect("should deserialize");
        assert_eq!(args.address, "test");
        assert!(!args.recurse);
    }

    #[test]
    fn args_into_lore_conversion() {
        let args = StoreImmutableQueryArgs {
            address: "deadbeef".into(),
            recurse: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.address.as_str(), "deadbeef");
        assert_eq!(lore_args.recurse, 1);
    }

    #[test]
    fn args_into_lore_no_recurse() {
        let args = StoreImmutableQueryArgs {
            address: "abc".into(),
            recurse: false,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.recurse, 0);
    }

    #[test]
    fn result_serializes() {
        let result = StoreImmutableQueryResult {
            entries: vec![StoreImmutableQueryEntry {
                address: "abc123".into(),
                remote: false,
                status: 0,
                status_label: "stored".into(),
                payload: true,
                subfragment: false,
                flags: 0x1,
                payload_size: 1024,
                content_size: 2048,
            }],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("abc123"));
        assert!(json.contains("stored"));
        assert!(json.contains("1024"));
    }

    #[test]
    fn empty_result_serializes() {
        let result = StoreImmutableQueryResult { entries: vec![] };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("[]"));
    }

    #[test]
    fn status_label_values() {
        assert_eq!(status_label(0), "stored");
        assert_eq!(status_label(1), "hash_exists");
        assert_eq!(status_label(2), "hash_exists_other_repo");
        assert_eq!(status_label(3), "not_found");
        assert_eq!(status_label(99), "unknown");
    }
}
