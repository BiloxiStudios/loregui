//! `file metadata_clear` operation — binds `lore::file::metadata_clear`.
//!
//! Clears all metadata associated with a file. Use `metadata_get` to read
//! metadata and `metadata_set` to write it; `metadata_clear` removes all
//! metadata keys for a file at once.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileMetadataClearArgs;
use lore::interface::LoreString;
use serde::{Deserialize, Serialize};

/// Arguments for [`metadata_clear`].
///
/// Mirrors `LoreFileMetadataClearArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataClearArgs {
    /// Path to the file whose metadata will be cleared.
    pub path: String,
}

impl MetadataClearArgs {
    fn into_lore(self) -> LoreFileMetadataClearArgs {
        LoreFileMetadataClearArgs {
            path: LoreString::from_str(&self.path),
        }
    }
}

/// Result returned on successful metadata clear.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataClearResult {
    /// The file path whose metadata was cleared.
    pub path: String,
}

/// Clear all metadata associated with a file.
///
/// Calls the upstream `lore::file::metadata_clear` in-process and returns
/// a typed result indicating which file's metadata was cleared.
pub async fn metadata_clear(api: &LoreApi, args: MetadataClearArgs) -> Result<MetadataClearResult> {
    let path = args.path.clone();

    let (callback, rx) = collect_events();

    let status =
        lore::file::metadata_clear(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("metadata_clear failed with status {status}"),
        )));
    }

    Ok(MetadataClearResult { path })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_clear_args_serializes() {
        let args = MetadataClearArgs {
            path: "file.txt".to_string(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("file.txt"));
    }

    #[test]
    fn metadata_clear_args_deserializes() {
        let json = r#"{"path":"file.txt"}"#;
        let args: MetadataClearArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.path, "file.txt");
    }

    #[test]
    fn metadata_clear_args_into_lore() {
        let args = MetadataClearArgs {
            path: "test/file.txt".to_string(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.path.as_str(), "test/file.txt");
    }

    #[test]
    fn metadata_clear_result_serializes() {
        let result = MetadataClearResult {
            path: "file.txt".to_string(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("file.txt"));
    }

    #[test]
    fn metadata_clear_result_deserializes() {
        let json = r#"{"path":"file.txt"}"#;
        let result: MetadataClearResult = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(result.path, "file.txt");
    }
}
