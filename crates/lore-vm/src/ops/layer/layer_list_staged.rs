//! `layer layer_list_staged` operation — binds `lore::layer::layer_list_staged`.
//!
//! Lists layers with staged changes in the current repository.
//! Calls [`lore::layer::layer_list_staged`] in-process (no CLI shelling) and collects
//! `LayerStagedEntry` events to return typed results.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::layer::LoreLayerListStagedArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`layer_list_staged`].
///
/// Mirrors `LoreLayerListStagedArgs` from the upstream `lore` crate (empty struct).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayerListStagedArgs;

impl LayerListStagedArgs {
    fn into_lore(self) -> LoreLayerListStagedArgs {
        LoreLayerListStagedArgs {}
    }
}

/// A single staged-layer entry returned by `layer list_staged`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedLayerEntry {
    /// Path in the outer repository where the layer is placed.
    pub target_path: String,
    /// Identifier of the source repository.
    pub source_repository: String,
    /// Number of staged files inside the layer.
    pub staged_file_count: u64,
}

/// Result of a successful `layer list_staged` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerListStagedResult {
    /// Number of layers with staged changes.
    pub layer_count: u32,
    /// Details of each layer with staged changes.
    pub layers: Vec<StagedLayerEntry>,
}

/// List all layers with staged changes in the current repository.
///
/// Calls upstream `lore::layer::layer_list_staged` in-process, collects the
/// `LayerStagedEntry` events emitted for each layer with staged changes,
/// and returns a typed result.
pub async fn layer_list_staged(
    api: &LoreApi,
    args: LayerListStagedArgs,
) -> Result<LayerListStagedResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::layer::layer_list_staged(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("layer list_staged failed with status {status}"),
        )));
    }

    let mut layers = Vec::new();
    for event in &stream.events {
        if let LoreEvent::LayerStagedEntry(data) = event {
            layers.push(StagedLayerEntry {
                target_path: data.target_path.as_str().to_string(),
                source_repository: format!("{}", data.source_repository),
                staged_file_count: data.staged_file_count,
            });
        }
    }

    Ok(LayerListStagedResult {
        layer_count: layers.len() as u32,
        layers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_layer_entry_serializes() {
        let entry = StagedLayerEntry {
            target_path: "/layers/assets".into(),
            source_repository: "city-of-brains".into(),
            staged_file_count: 5,
        };

        assert_eq!(entry.target_path, "/layers/assets");
        assert_eq!(entry.source_repository, "city-of-brains");
        assert_eq!(entry.staged_file_count, 5);

        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("\"target_path\":\"/layers/assets\""));
        assert!(json.contains("\"source_repository\":\"city-of-brains\""));
        assert!(json.contains("\"staged_file_count\":5"));
    }

    #[test]
    fn layer_list_staged_result_serialization() {
        let result = LayerListStagedResult {
            layer_count: 2,
            layers: vec![
                StagedLayerEntry {
                    target_path: "/layers/assets".into(),
                    source_repository: "city-of-brains".into(),
                    staged_file_count: 5,
                },
                StagedLayerEntry {
                    target_path: "/layers/world".into(),
                    source_repository: "world-builder".into(),
                    staged_file_count: 3,
                },
            ],
        };

        assert_eq!(result.layer_count, 2);
        assert_eq!(result.layers.len(), 2);

        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: LayerListStagedResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.layer_count, 2);
        assert_eq!(deserialized.layers[0].target_path, "/layers/assets");
        assert_eq!(deserialized.layers[1].source_repository, "world-builder");
    }

    #[test]
    fn empty_layer_list_staged_result() {
        let result = LayerListStagedResult {
            layer_count: 0,
            layers: vec![],
        };

        assert_eq!(result.layer_count, 0);
        assert!(result.layers.is_empty());

        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("\"layer_count\":0"));
        assert!(json.contains("\"layers\":[]"));
    }

    #[test]
    fn staged_layer_entry_with_zero_files() {
        let entry = StagedLayerEntry {
            target_path: "/layers/empty".into(),
            source_repository: "empty-repo".into(),
            staged_file_count: 0,
        };

        assert_eq!(entry.staged_file_count, 0);

        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("\"staged_file_count\":0"));
    }

    #[test]
    fn layer_list_staged_args_defaults() {
        let args = LayerListStagedArgs;
        let lore_args = args.into_lore();
        // Just verify it compiles - LoreLayerListStagedArgs is an empty struct
        let _ = lore_args;
    }
}
