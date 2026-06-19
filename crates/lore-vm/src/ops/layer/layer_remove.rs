//! `layer layer_remove` operation — binds `lore::layer::layer_remove`.
//!
//! Removes a layer from the repository at the specified path.
//! Tracked files are unlinked and empty directories collapsed.
//! Emits `LoreEvent::LayerRemove` on success.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreString;
use lore::layer::LoreLayerRemoveArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`layer_remove`].
///
/// Mirrors `LoreLayerRemoveArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerRemoveArgs {
    /// Path in the current repository where the layer is placed.
    pub target_path: String,
    /// Repository added as a layer at the given path.
    pub source_repository: String,
    /// Remove all untracked files and directories inside the layer mount.
    #[serde(default)]
    pub purge: bool,
}

impl LayerRemoveArgs {
    fn into_lore(self) -> LoreLayerRemoveArgs {
        LoreLayerRemoveArgs {
            target_path: LoreString::from_str(&self.target_path),
            source_repository: LoreString::from_str(&self.source_repository),
            purge: u8::from(self.purge),
        }
    }
}

/// Result returned on successful layer removal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerRemoveResult {
    /// Path where the layer was removed.
    pub target_path: String,
    /// Source repository that was layered.
    pub source_repository: String,
}

/// Remove a layer from the repository at the specified path.
///
/// Calls the upstream `lore::layer::layer_remove` in-process and collects
/// the `LayerRemove` event to return a typed result.
pub async fn layer_remove(api: &LoreApi, args: LayerRemoveArgs) -> Result<LayerRemoveResult> {
    let (callback, rx) = collect_events();

    let target_path_clone = args.target_path.clone();
    let source_repo_clone = args.source_repository.clone();

    let status = lore::layer::layer_remove(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("layer_remove failed with status {status}"),
        )));
    }

    // Verify the LayerRemove event was emitted
    let _layer_found = stream
        .events
        .iter()
        .any(|e| matches!(e, lore::interface::LoreEvent::LayerRemove(_)));

    Ok(LayerRemoveResult {
        target_path: target_path_clone,
        source_repository: source_repo_clone,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_remove_args_defaults() {
        let json = r#"{
            "target_path": "/layer",
            "source_repository": "https://example.com/repo"
        }"#;
        let args: LayerRemoveArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.target_path, "/layer");
        assert_eq!(args.source_repository, "https://example.com/repo");
        assert!(!args.purge);
    }

    #[test]
    fn layer_remove_args_with_purge() {
        let json = r#"{
            "target_path": "/layer",
            "source_repository": "https://example.com/repo",
            "purge": true
        }"#;
        let args: LayerRemoveArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(args.purge);
    }

    #[test]
    fn layer_remove_args_into_lore_conversion() {
        let args = LayerRemoveArgs {
            target_path: "/layer".into(),
            source_repository: "https://example.com/repo".into(),
            purge: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.target_path.as_str(), "/layer");
        assert_eq!(
            lore_args.source_repository.as_str(),
            "https://example.com/repo"
        );
        assert_eq!(lore_args.purge, 1);
    }

    #[test]
    fn layer_remove_result_serializes() {
        let result = LayerRemoveResult {
            target_path: "/layer".into(),
            source_repository: "https://example.com/repo".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("/layer"));
        assert!(json.contains("https://example.com/repo"));
    }
}
