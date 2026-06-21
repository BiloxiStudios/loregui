//! `revision metadata_clear` operation — binds `lore::revision::metadata_clear`.
//!
//! Clears ALL metadata from the current revision's staged state.
//! Use `metadata_get` to read keys and `metadata_set` to write them;
//! `metadata_clear` removes all metadata at once.
//!
//! NOTE: The upstream lore API takes no arguments and clears all metadata.
//! The frontend manifest's `keys` and `revision` parameters are ignored
//! (manifest designed for a more flexible API that doesn't exist upstream).

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::revision::LoreRevisionMetadataClearArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`metadata_clear`].
///
/// NOTE: Upstream lore takes no arguments, but we provide fields for
/// compatibility with the frontend manifest. These fields are ignored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionMetadataClearArgs {
    /// IGNORED: Upstream lore clears all metadata, not selective keys.
    #[serde(default)]
    pub keys: Vec<String>,
    /// IGNORED: Upstream lore always targets the current staged revision.
    #[serde(default)]
    pub revision: String,
}

impl RevisionMetadataClearArgs {
    fn into_lore(self) -> LoreRevisionMetadataClearArgs {
        // Upstream LoreRevisionMetadataClearArgs is an empty struct
        LoreRevisionMetadataClearArgs {}
    }
}

/// Result returned on successful metadata clear.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionMetadataClearResult {
    /// Hash of the revision whose metadata was cleared.
    pub revision: String,
    /// Indicates success (true when metadata was cleared).
    pub success: bool,
}

/// Clear ALL metadata from the current revision's staged state.
///
/// Calls the upstream `lore::revision::metadata_clear` in-process and returns
/// a typed result with the revision hash. NOTE: This clears ALL metadata,
/// not selective keys — the upstream API doesn't support selective clearing.
pub async fn metadata_clear(
    api: &LoreApi,
    args: RevisionMetadataClearArgs,
) -> Result<RevisionMetadataClearResult> {
    let _ = args; // Fields are ignored, but we accept them for manifest compatibility

    let (callback, rx) = collect_events();

    let status =
        lore::revision::metadata_clear(api.globals().build(), LoreRevisionMetadataClearArgs {}, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision metadata_clear failed with status {status}"),
        )));
    }

    // Extract the revision hash from MetadataClearRevision event
    let revision = stream
        .events
        .iter()
        .find_map(|event| {
            if let lore::interface::LoreEvent::MetadataClearRevision(data) = event {
                Some(format!("{}", data.revision))
            } else {
                None
            }
        })
        .unwrap_or_else(|| String::from("unknown"));

    Ok(RevisionMetadataClearResult {
        revision,
        success: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes_with_ignored_fields() {
        let args = RevisionMetadataClearArgs {
            keys: vec!["change-request".into()],
            revision: "abc123".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("change-request"));
        assert!(json.contains("abc123"));
    }

    #[test]
    fn args_deserializes_with_defaults() {
        let json = r#"{}"#;
        let args: RevisionMetadataClearArgs =
            serde_json::from_str(json).expect("should deserialize with defaults");
        assert!(args.keys.is_empty());
        assert_eq!(args.revision, "");
    }

    #[test]
    fn args_into_lore_ignores_fields() {
        let args = RevisionMetadataClearArgs {
            keys: vec!["tag".into()],
            revision: "deadbeef".into(),
        };
        let _lore_args = args.into_lore();
        // LoreRevisionMetadataClearArgs is empty, so no fields to check
    }

    #[test]
    fn result_serializes() {
        let result = RevisionMetadataClearResult {
            revision: "abc123def".into(),
            success: true,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("abc123def"));
        assert!(json.contains("true"));
    }

    #[test]
    fn serde_roundtrip() {
        let args = RevisionMetadataClearArgs {
            keys: vec!["ignored".into()],
            revision: "also-ignored".into(),
        };
        let json = serde_json::to_string(&args).expect("serialize");
        let deser: RevisionMetadataClearArgs = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.keys, vec!["ignored"]);
        assert_eq!(deser.revision, "also-ignored");
    }
}
