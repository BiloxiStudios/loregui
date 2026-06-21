//! `revision metadata_clear` operation — binds `lore::revision::metadata_clear`.
//!
//! Clears all metadata from the current revision. Takes no arguments beyond
//! the repository context. Use `metadata_get` to read keys and `metadata_set`
//! to write them; `metadata_clear` removes them entirely.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::revision::LoreRevisionMetadataClearArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`metadata_clear`].
///
/// The upstream `LoreRevisionMetadataClearArgs` is an empty struct —
/// only the repository context (provided by `LoreApi`) is needed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataClearArgs {}

impl MetadataClearArgs {
    fn into_lore(self) -> LoreRevisionMetadataClearArgs {
        LoreRevisionMetadataClearArgs {}
    }
}

/// Result returned on successful metadata clear.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataClearResult {
    /// Whether the metadata was cleared successfully.
    pub cleared: bool,
}

/// Clear all metadata from the current revision.
///
/// Calls the upstream `lore::revision::metadata_clear` in-process and checks
/// the completion status.
pub async fn metadata_clear(api: &LoreApi, args: MetadataClearArgs) -> Result<MetadataClearResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::revision::metadata_clear(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("metadata_clear failed with status {status}"),
        )));
    }

    Ok(MetadataClearResult { cleared: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises() {
        let args = MetadataClearArgs {};
        let json = serde_json::to_string(&args).expect("should serialise");
        assert_eq!(json, "{}");
    }

    #[test]
    fn args_deserialises() {
        let json = "{}";
        let _args: MetadataClearArgs = serde_json::from_str(json).expect("should deserialise");
    }

    #[test]
    fn args_into_lore_conversion() {
        let args = MetadataClearArgs {};
        let _lore_args = args.into_lore();
    }

    #[test]
    fn result_serialises() {
        let result = MetadataClearResult { cleared: true };
        let json = serde_json::to_string(&result).expect("should serialise");
        assert!(json.contains("true"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"cleared": true}"#;
        let result: MetadataClearResult = serde_json::from_str(json).expect("should deserialise");
        assert!(result.cleared);
    }

    #[test]
    fn args_default() {
        let args = MetadataClearArgs::default();
        let json = serde_json::to_string(&args).expect("should serialise");
        assert_eq!(json, "{}");
    }
}
