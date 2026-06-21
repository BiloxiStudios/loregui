//! `revision metadata_clear` operation — binds `lore::revision::metadata_clear`.
//!
//! Clears all user-defined metadata from the current revision. The upstream
//! `LoreRevisionMetadataClearArgs` takes no fields: the operation always clears
//! every metadata key on the current revision (it does not select individual
//! keys). Use `metadata_get`/`metadata_list` to read keys and `metadata_set` to
//! write them; `metadata_clear` removes them entirely.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::revision::LoreRevisionMetadataClearArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`metadata_clear`].
///
/// Mirrors `LoreRevisionMetadataClearArgs` from the upstream `lore` crate, which
/// is empty — the operation clears all metadata on the current revision.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataClearArgs {}

impl MetadataClearArgs {
    fn into_lore(self) -> LoreRevisionMetadataClearArgs {
        LoreRevisionMetadataClearArgs {}
    }
}

/// Result returned on successful metadata clear.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataClearResult {
    /// True when the clear completed successfully.
    pub cleared: bool,
}

/// Clear all metadata from the current revision.
///
/// Calls the upstream `lore::revision::metadata_clear` in-process and returns a
/// typed result indicating the clear succeeded.
pub async fn metadata_clear(api: &LoreApi, args: MetadataClearArgs) -> Result<MetadataClearResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::revision::metadata_clear(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision metadata_clear failed with status {status}"),
        )));
    }

    // The upstream op clears every key on the current revision and emits a
    // `MetadataClearRevision` event on success; there is no per-key payload to
    // surface, so we report a simple success flag.
    Ok(MetadataClearResult { cleared: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_default_serializes() {
        let args = MetadataClearArgs::default();
        let json = serde_json::to_string(&args).expect("should serialize");
        assert_eq!(json, "{}");
    }

    #[test]
    fn args_deserializes_empty() {
        let args: MetadataClearArgs =
            serde_json::from_str("{}").expect("should deserialize empty object");
        let _ = args.into_lore();
    }

    #[test]
    fn result_serializes() {
        let result = MetadataClearResult { cleared: true };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("cleared"));
        assert!(json.contains("true"));
    }

    #[test]
    fn result_roundtrip() {
        let result = MetadataClearResult { cleared: true };
        let json = serde_json::to_string(&result).expect("serialize");
        let deser: MetadataClearResult = serde_json::from_str(&json).expect("deserialize");
        assert!(deser.cleared);
    }
}
