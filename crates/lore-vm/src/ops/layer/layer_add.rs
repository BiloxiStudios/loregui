//! `layer layer_add` operation — binds `lore::layer::layer_add`.
//!
//! Adds a layer from a source repository into the current repository at the
//! specified path.
//! Emits `LoreEvent::LayerAdd` on success.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::interface::LoreString;
use lore::layer::LoreLayerAddArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`layer_add`].
///
/// Mirrors `LoreLayerAddArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerAddArgs {
    /// Path in the current repository where the layer should be placed.
    pub target_path: String,
    /// Repository to add as a layer.
    pub source_repository: String,
    /// Path in the layer repository where the layer should start.
    pub source_path: String,
    /// Metadata key to use to match revisions.
    #[serde(default)]
    pub metadata: String,
}

impl LayerAddArgs {
    #[must_use]
    fn into_lore(self) -> LoreLayerAddArgs {
        LoreLayerAddArgs {
            target_path: LoreString::from_str(&self.target_path),
            source_repository: LoreString::from_str(&self.source_repository),
            source_path: LoreString::from_str(&self.source_path),
            metadata: LoreString::from_str(&self.metadata),
        }
    }
}

/// Result returned on successful layer add.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerAddResult {
    /// Path in the outer repository where the layer is placed.
    pub target_path: String,
    /// Identifier of the source repository.
    pub source_repository: String,
    /// Path inside the source repository where the layer starts.
    pub source_path: String,
    /// Metadata used to match revisions between repositories.
    pub metadata: String,
    /// Revision of the source repository.
    pub revision: String,
}

/// Add a layer from a source repository into the current repository.
///
/// Calls the upstream `lore::layer::layer_add` in-process and collects
/// the `LayerAdd` event to return a typed result.
pub async fn layer_add(api: &LoreApi, args: LayerAddArgs) -> Result<LayerAddResult> {
    let (callback, rx) = collect_events();

    let status = lore::layer::layer_add(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("layer_add failed with status {status}"),
        )));
    }

    for event in &stream.events {
        if let LoreEvent::LayerAdd(data) = event {
            return Ok(LayerAddResult {
                target_path: data.target_path.to_string(),
                source_repository: data.source_repository.to_string(),
                source_path: data.source_path.to_string(),
                metadata: data.metadata.to_string(),
                revision: data.revision.to_string(),
            });
        }
    }

    Err(LoreError::Parse(
        "layer_add succeeded but no LayerAdd event emitted".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_add_args_deserializes_with_metadata_default() {
        let json = r#"{
            "target_path": "/layers/world",
            "source_repository": "city-of-brains",
            "source_path": "/"
        }"#;

        let args: LayerAddArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.target_path, "/layers/world");
        assert_eq!(args.source_repository, "city-of-brains");
        assert_eq!(args.source_path, "/");
        assert!(args.metadata.is_empty());
    }

    #[test]
    fn layer_add_args_into_lore_conversion() {
        let args = LayerAddArgs {
            target_path: "/layers/world".into(),
            source_repository: "city-of-brains".into(),
            source_path: "/maps".into(),
            metadata: "branch".into(),
        };

        let lore_args = args.into_lore();
        assert_eq!(lore_args.target_path.as_str(), "/layers/world");
        assert_eq!(lore_args.source_repository.as_str(), "city-of-brains");
        assert_eq!(lore_args.source_path.as_str(), "/maps");
        assert_eq!(lore_args.metadata.as_str(), "branch");
    }

    #[test]
    fn layer_add_result_serializes() {
        let result = LayerAddResult {
            target_path: "/layers/world".into(),
            source_repository: "city-of-brains".into(),
            source_path: "/maps".into(),
            metadata: "branch".into(),
            revision: "aabbccddeeff00112233445566778899".into(),
        };

        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("/layers/world"));
        assert!(json.contains("city-of-brains"));
        assert!(json.contains("aabbccddeeff00112233445566778899"));
    }

    #[test]
    fn layer_add_result_round_trips() {
        let json = r#"{
            "target_path":"/layers/world",
            "source_repository":"city-of-brains",
            "source_path":"/maps",
            "metadata":"branch",
            "revision":"abc123"
        }"#;

        let result: LayerAddResult = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(result.target_path, "/layers/world");
        assert_eq!(result.source_repository, "city-of-brains");
        assert_eq!(result.source_path, "/maps");
        assert_eq!(result.metadata, "branch");
        assert_eq!(result.revision, "abc123");
    }
}
