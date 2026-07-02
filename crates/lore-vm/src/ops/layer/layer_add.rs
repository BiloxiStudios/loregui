//! `layer layer_add` operation — binds `lore::layer::layer_add`.
//!
//! Adds a layer from a source repository into the current repository at the
//! given target path. The source repository is mounted starting at `source_path`
//! and revisions are matched between the two repositories via the `metadata` key.
//! Emits `LoreEvent::LayerAdd` on success.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::layer::LoreLayerAddArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`layer_add`].
///
/// Mirrors `LoreLayerAddArgs` from the upstream `lore` crate but uses plain
/// `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerAddArgs {
    /// Path in the current repository where the layer should be placed.
    pub target_path: String,
    /// Repository to add as a layer.
    pub source_repository: String,
    /// Path in the layer repository where the layer should start.
    #[serde(default)]
    pub source_path: String,
    /// Metadata key used to match revisions between the repositories.
    #[serde(default)]
    pub metadata: String,
}

impl LayerAddArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreLayerAddArgs {
        LoreLayerAddArgs {
            target_path: crate::ops::paths::lore_path_arg(repo_root, &self.target_path),
            source_repository: LoreString::from_str(&self.source_repository),
            source_path: LoreString::from_str(&self.source_path),
            metadata: LoreString::from_str(&self.metadata),
        }
    }
}

/// Result returned on successful layer addition.
///
/// Populated from the `LayerAdd` event emitted by the upstream op.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayerAddResult {
    /// Path in the outer repository where the layer was placed.
    pub target_path: String,
    /// Identifier of the source repository.
    pub source_repository: String,
    /// Path inside the source repository where the layer starts.
    pub source_path: String,
    /// Metadata key used to match revisions between the repositories.
    pub metadata: String,
    /// Revision of the source repository that was layered in.
    pub revision: String,
}

/// Add a layer from a source repository into the current repository.
///
/// Calls the upstream `lore::layer::layer_add` in-process and collects the
/// `LayerAdd` event to return a typed result.
pub async fn layer_add(api: &LoreApi, args: LayerAddArgs) -> Result<LayerAddResult> {
    let (callback, rx) = collect_events();

    // Retain the requested values as a fallback in case the event omits any.
    let target_path_req = args.target_path.clone();
    let source_repo_req = args.source_repository.clone();
    let source_path_req = args.source_path.clone();
    let metadata_req = args.metadata.clone();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::layer::layer_add(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("layer_add failed with status {status}"),
        )));
    }

    let result = stream.events.iter().find_map(|event| {
        if let LoreEvent::LayerAdd(data) = event {
            Some(LayerAddResult {
                target_path: data.target_path.as_str().to_string(),
                source_repository: format!("{}", data.source_repository),
                source_path: data.source_path.as_str().to_string(),
                metadata: data.metadata.as_str().to_string(),
                revision: format!("{}", data.revision),
            })
        } else {
            None
        }
    });

    // No LayerAdd event (op succeeded but emitted nothing): echo the request.
    Ok(result.unwrap_or(LayerAddResult {
        target_path: target_path_req,
        source_repository: source_repo_req,
        source_path: source_path_req,
        metadata: metadata_req,
        revision: String::new(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_add_args_deserializes() {
        let json = r#"{
            "target_path": "/layers/assets",
            "source_repository": "https://example.com/repo",
            "source_path": "/",
            "metadata": "branch"
        }"#;
        let args: LayerAddArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.target_path, "/layers/assets");
        assert_eq!(args.source_repository, "https://example.com/repo");
        assert_eq!(args.source_path, "/");
        assert_eq!(args.metadata, "branch");
    }

    #[test]
    fn layer_add_args_defaults_optional_fields() {
        let json = r#"{
            "target_path": "/layers/assets",
            "source_repository": "https://example.com/repo"
        }"#;
        let args: LayerAddArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.source_path, "");
        assert_eq!(args.metadata, "");
    }

    #[test]
    fn layer_add_args_into_lore_conversion() {
        let args = LayerAddArgs {
            target_path: "/layers/assets".into(),
            source_repository: "https://example.com/repo".into(),
            source_path: "/".into(),
            metadata: "branch".into(),
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.target_path.as_str(), "/layers/assets");
        assert_eq!(
            lore_args.source_repository.as_str(),
            "https://example.com/repo"
        );
        assert_eq!(lore_args.source_path.as_str(), "/");
        assert_eq!(lore_args.metadata.as_str(), "branch");
    }

    #[test]
    fn layer_add_target_path_relative_joined() {
        let args = LayerAddArgs {
            target_path: "layers/assets".into(),
            source_repository: "https://example.com/repo".into(),
            source_path: "/src".into(),
            metadata: String::new(),
        };
        let lore_args = args.into_lore(std::path::Path::new("/project"));
        assert_eq!(lore_args.target_path.as_str(), "/project/layers/assets");
    }

    #[test]
    fn layer_add_target_path_empty_preserved() {
        let args = LayerAddArgs {
            target_path: String::new(),
            source_repository: "https://example.com/repo".into(),
            source_path: "/src".into(),
            metadata: String::new(),
        };
        let lore_args = args.into_lore(std::path::Path::new("/project"));
        assert_eq!(
            lore_args.target_path.as_str(),
            "",
            "empty target_path must stay empty"
        );
    }

    #[test]
    fn layer_add_result_serializes() {
        let result = LayerAddResult {
            target_path: "/layers/assets".into(),
            source_repository: "repo-id".into(),
            source_path: "/".into(),
            metadata: "branch".into(),
            revision: "abc123".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("/layers/assets"));
        assert!(json.contains("repo-id"));
        assert!(json.contains("abc123"));
    }
}
