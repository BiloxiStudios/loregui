//! `layer layer_list` operation — binds `lore::layer::layer_list`.
//!
//! Lists all layers configured in the repository, emitting a `LayerEntry` event
//! per layer with the target path, source repository, source path, metadata key,
//! and current revision.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::layer::LoreLayerListArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`layer_list`].
///
/// Mirrors `LoreLayerListArgs` from the upstream `lore` crate (empty struct).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayerListArgs;

impl LayerListArgs {
    fn into_lore(self) -> LoreLayerListArgs {
        LoreLayerListArgs {}
    }
}

/// One layer entry returned by `layer_list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerEntry {
    /// Path in the outer repository where the layer is placed.
    pub target_path: String,
    /// Identifier of the source repository.
    pub source_repository: String,
    /// Path inside the source repository where the layer starts.
    pub source_path: String,
    /// Metadata key used to match revisions between repositories.
    pub metadata: String,
    /// Current revision of the source repository layer.
    pub revision: String,
}

/// Result returned on successful layer list.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayerListResult {
    /// One entry per layer configured in the repository.
    pub layers: Vec<LayerEntry>,
}

/// List all layers configured in the repository.
///
/// Calls the upstream `lore::layer::layer_list` in-process and collects the
/// `LayerEntry` events into a typed result.
pub async fn layer_list(api: &LoreApi, args: LayerListArgs) -> Result<LayerListResult> {
    let (callback, rx) = collect_events();

    let status = lore::layer::layer_list(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("layer_list failed with status {status}"),
        )));
    }

    let mut result = LayerListResult::default();

    for event in &stream.events {
        if let LoreEvent::LayerEntry(data) = event {
            result.layers.push(LayerEntry {
                target_path: data.target_path.as_str().to_string(),
                source_repository: format!("{}", data.source_repository),
                source_path: data.source_path.as_str().to_string(),
                metadata: data.metadata.as_str().to_string(),
                revision: format!("{}", data.revision),
            });
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_list_args_defaults() {
        let args = LayerListArgs;
        let lore_args = args.into_lore();
        // Just verify it compiles - LoreLayerListArgs is an empty struct
        let _ = lore_args;
    }

    #[test]
    fn layer_entry_serializes() {
        let entry = LayerEntry {
            target_path: "/layers/assets".to_string(),
            source_repository: "repo-id".to_string(),
            source_path: "/".to_string(),
            metadata: "branch".to_string(),
            revision: "abc123".to_string(),
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("/layers/assets"));
        assert!(json.contains("repo-id"));
    }

    #[test]
    fn layer_list_result_default_is_empty() {
        let result = LayerListResult::default();
        assert!(result.layers.is_empty());
    }
}
