//! `repository metadata_clear` operation — binds `lore::repository::metadata_clear`.
//!
//! Clears metadata keys from the current repository. Use `metadata_get` to read
//! keys and `metadata_set` to write them; `metadata_clear` removes them entirely.
//! Passing an empty `keys` array clears all user-defined keys.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreString;
use lore::repository::LoreRepositoryMetadataClearArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`metadata_clear`].
///
/// Mirrors `LoreRepositoryMetadataClearArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryMetadataClearArgs {
    /// Metadata keys to clear. An empty array clears all user-defined keys.
    #[serde(default)]
    pub keys: Vec<String>,
}

impl RepositoryMetadataClearArgs {
    fn into_lore(self) -> LoreRepositoryMetadataClearArgs {
        LoreRepositoryMetadataClearArgs {
            keys: lore::interface::LoreArray::from_vec(
                self.keys
                    .into_iter()
                    .map(|k| LoreString::from_str(&k))
                    .collect(),
            ),
        }
    }
}

/// Result returned on successful metadata clear.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryMetadataClearResult {
    /// The keys that were cleared.
    pub keys: Vec<String>,
}

/// Clear metadata keys from the current repository.
///
/// Calls the upstream `lore::repository::metadata_clear` in-process and returns
/// a typed result indicating which keys were cleared.
pub async fn metadata_clear(
    api: &LoreApi,
    args: RepositoryMetadataClearArgs,
) -> Result<RepositoryMetadataClearResult> {
    let keys = args.keys.clone();

    let (callback, rx) = collect_events();

    let status =
        lore::repository::metadata_clear(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("repository metadata_clear failed with status {status}"),
        )));
    }

    Ok(RepositoryMetadataClearResult { keys })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes() {
        let args = RepositoryMetadataClearArgs {
            keys: vec!["description".into(), "owner".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("description"));
        assert!(json.contains("owner"));
    }

    #[test]
    fn args_deserializes() {
        let json = r#"{"keys":["description","owner"]}"#;
        let args: RepositoryMetadataClearArgs =
            serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.keys, vec!["description", "owner"]);
    }

    #[test]
    fn args_deserializes_empty_keys() {
        let json = r#"{}"#;
        let args: RepositoryMetadataClearArgs =
            serde_json::from_str(json).expect("should deserialize with default");
        assert!(args.keys.is_empty());
    }

    #[test]
    fn args_into_lore() {
        let args = RepositoryMetadataClearArgs {
            keys: vec!["tag".into(), "version".into()],
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.keys.as_slice().len(), 2);
        assert_eq!(lore_args.keys.as_slice()[0].as_str(), "tag");
        assert_eq!(lore_args.keys.as_slice()[1].as_str(), "version");
    }

    #[test]
    fn result_serializes() {
        let result = RepositoryMetadataClearResult {
            keys: vec!["description".into()],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("description"));
    }

    #[test]
    fn result_deserializes() {
        let json = r#"{"keys":["description"]}"#;
        let result: RepositoryMetadataClearResult =
            serde_json::from_str(json).expect("should deserialize");
        assert_eq!(result.keys, vec!["description"]);
    }

    #[test]
    fn serde_roundtrip() {
        let args = RepositoryMetadataClearArgs {
            keys: vec!["tag".into(), "owner".into()],
        };
        let json = serde_json::to_string(&args).expect("serialize");
        let deser: RepositoryMetadataClearArgs = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.keys, vec!["tag", "owner"]);
    }
}
