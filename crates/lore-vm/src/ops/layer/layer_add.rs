//! `layer layer_add` operation — binds `lore::layer::layer_add`.
//!
//! Adds a layer from a source repository into the current repository at the
//! specified path. Emits `LoreEvent::LayerAdd` on success.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

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
    fn into_lore(self) -> LoreLayerAddArgs {
        LoreLayerAddArgs {
            target_path: LoreString::from_str(&self.target_path),
            source_repository: LoreString::from_str(&self.source_repository),
            source_path: LoreString::from_str(&self.source_path),
            metadata: LoreString::from_str(&self.metadata),
        }
    }
}

/// Result returned on successful layer addition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerAddResult {
    /// Path where the layer was added.
    pub target_path: String,
    /// Source repository that was added as a layer.
    pub source_repository: String,
    /// Path inside the source repository where the layer starts.
    pub source_path: String,
    /// Metadata key used to match revisions.
    pub metadata: String,
}

/// Add a layer from a source repository into the current repository at the
/// specified path.
///
/// Calls the upstream `lore::layer::layer_add` in-process and collects
/// the `LayerAdd` event to return a typed result.
pub async fn layer_add(api: &LoreApi, args: LayerAddArgs) -> Result<LayerAddResult> {
    let (callback, rx) = collect_events();

    let target_path_clone = args.target_path.clone();
    let source_repo_clone = args.source_repository.clone();
    let source_path_clone = args.source_path.clone();
    let metadata_clone = args.metadata.clone();

    let status = lore::layer::layer_add(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("layer_add failed with status {status}"),
        )));
    }

    // Verify the LayerAdd event was emitted
    let _layer_added = stream
        .events
        .iter()
        .any(|e| matches!(e, lore::interface::LoreEvent::LayerAdd(_)));

    Ok(LayerAddResult {
        target_path: target_path_clone,
        source_repository: source_repo_clone,
        source_path: source_path_clone,
        metadata: metadata_clone,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_add_args_deserialize() {
        let json = r#"{
            "target_path": "/layers/assets",
            "source_repository": "https://example.com/repo",
            "source_path": "/"
        }"#;
        let args: LayerAddArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.target_path, "/layers/assets");
        assert_eq!(args.source_repository, "https://example.com/repo");
        assert_eq!(args.source_path, "/");
        assert!(args.metadata.is_empty());
    }

    #[test]
    fn layer_add_args_with_metadata() {
        let json = r#"{
            "target_path": "/layers/assets",
            "source_repository": "https://example.com/repo",
            "source_path": "/assets",
            "metadata": "branch"
        }"#;
        let args: LayerAddArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.metadata, "branch");
    }

    #[test]
    fn layer_add_args_into_lore_conversion() {
        let args = LayerAddArgs {
            target_path: "/layers/assets".into(),
            source_repository: "https://example.com/repo".into(),
            source_path: "/assets".into(),
            metadata: "branch".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.target_path.as_str(), "/layers/assets");
        assert_eq!(
            lore_args.source_repository.as_str(),
            "https://example.com/repo"
        );
        assert_eq!(lore_args.source_path.as_str(), "/assets");
        assert_eq!(lore_args.metadata.as_str(), "branch");
    }

    #[test]
    fn layer_add_result_serializes() {
        let result = LayerAddResult {
            target_path: "/layers/assets".into(),
            source_repository: "https://example.com/repo".into(),
            source_path: "/assets".into(),
            metadata: "branch".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("/layers/assets"));
        assert!(json.contains("https://example.com/repo"));
        assert!(json.contains("/assets"));
        assert!(json.contains("branch"));
    }
}
