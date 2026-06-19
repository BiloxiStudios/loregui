//! `layer layer_add` operation — binds `lore::layer::layer_add`.
//!
//! Adds a layer from a source repository into the current repository at the
//! specified path. Emits `LayerAdd` on success containing layer details.

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
    #[serde(default)]
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
    /// Path in the outer repository where the layer is placed.
    pub target_path: String,
    /// Identifier of the source repository.
    pub source_repository: String,
    /// Path inside the source repository where the layer starts.
    pub source_path: String,
    /// Metadata used to match revisions between the repositories.
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

    let mut result: Option<LayerAddResult> = None;

    for event in &stream.events {
        if let LoreEvent::LayerAdd(data) = event {
            result = Some(LayerAddResult {
                target_path: data.target_path.as_str().to_string(),
                source_repository: format!("{}", data.source_repository),
                source_path: data.source_path.as_str().to_string(),
                metadata: data.metadata.as_str().to_string(),
                revision: format!("{}", data.revision),
            });
        }
    }

    result.ok_or_else(|| {
        LoreError::Parse("layer_add succeeded but no LayerAdd event emitted".into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_add_args_defaults() {
        let json = r#"{
            "target_path": "/layer",
            "source_repository": "https://example.com/repo"
        }"#;
        let args: LayerAddArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.target_path, "/layer");
        assert_eq!(args.source_repository, "https://example.com/repo");
        assert!(args.source_path.is_empty());
        assert!(args.metadata.is_empty());
    }

    #[test]
    fn layer_add_args_full() {
        let json = r#"{
            "target_path": "/layers/vendor",
            "source_repository": "https://example.com/repo.git",
            "source_path": "/assets",
            "metadata": "branch"
        }"#;
        let args: LayerAddArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.target_path, "/layers/vendor");
        assert_eq!(args.source_repository, "https://example.com/repo.git");
        assert_eq!(args.source_path, "/assets");
        assert_eq!(args.metadata, "branch");
    }

    #[test]
    fn layer_add_args_into_lore_conversion() {
        let args = LayerAddArgs {
            target_path: "/layers/vendor".into(),
            source_repository: "https://example.com/repo.git".into(),
            source_path: "/assets".into(),
            metadata: "branch".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.target_path.as_str(), "/layers/vendor");
        assert_eq!(lore_args.source_repository.as_str(), "https://example.com/repo.git");
        assert_eq!(lore_args.source_path.as_str(), "/assets");
        assert_eq!(lore_args.metadata.as_str(), "branch");
    }

    #[test]
    fn layer_add_result_serializes() {
        let result = LayerAddResult {
            target_path: "/layers/vendor".into(),
            source_repository: "https://example.com/repo.git".into(),
            source_path: "/assets".into(),
            metadata: "branch".into(),
            revision: "abc123".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("layers/vendor"));
        assert!(json.contains("assets"));
        assert!(json.contains("abc123"));
    }
}
