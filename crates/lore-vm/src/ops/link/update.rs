//! `link update` operation — binds `lore::link::update`.
//!
//! Updates a link's pin (branch/revision) for the linked repository at the
//! specified path. Calls [`lore::link::update`] in-process (no CLI shelling)
//! and collects `LinkChange` events to confirm the update.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreString;
use lore::link::LoreLinkUpdateArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`update`].
///
/// Mirrors `LoreLinkUpdateArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateArgs {
    /// Path within this repository where the link exists.
    pub link_path: String,
    /// New branch or revision to pin the link to.
    #[serde(default)]
    pub pin: String,
}

impl UpdateArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreLinkUpdateArgs {
        LoreLinkUpdateArgs {
            link_path: crate::ops::paths::lore_path_arg(repo_root, &self.link_path),
            pin: LoreString::from_str(&self.pin),
        }
    }
}

/// Result returned on successful `link update`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResult {
    /// The link path that was updated.
    pub link_path: String,
}

/// Updates a link's pin at the specified path.
///
/// Calls the upstream `lore::link::update` in-process and collects
/// events to confirm the link was updated.
pub async fn update(api: &LoreApi, args: UpdateArgs) -> Result<UpdateResult> {
    let link_path = args.link_path.clone();
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status = lore::link::update(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("link update failed with status {status}"),
        )));
    }

    let found_link_change = stream
        .events
        .iter()
        .any(|e| matches!(e, lore::interface::LoreEvent::LinkChange(_)));

    if !found_link_change {
        return Err(LoreError::Parse(
            "link update succeeded but no LinkChange event emitted".into(),
        ));
    }

    Ok(UpdateResult { link_path })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_args_serializes() {
        let args = UpdateArgs {
            link_path: "deps/external".into(),
            pin: "v2.0".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("deps/external"));
        assert!(json.contains("v2.0"));
    }

    #[test]
    fn update_args_deserializes_with_defaults() {
        let json = r#"{"link_path":"deps/external"}"#;
        let args: UpdateArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.link_path, "deps/external");
        assert_eq!(args.pin, "");
    }

    #[test]
    fn update_args_deserializes_full() {
        let json = r#"{"link_path":"deps/external","pin":"release/1.0"}"#;
        let args: UpdateArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.link_path, "deps/external");
        assert_eq!(args.pin, "release/1.0");
    }

    #[test]
    fn update_args_into_lore_conversion() {
        let args = UpdateArgs {
            link_path: "deps/external".into(),
            pin: "v2.0".into(),
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.link_path.as_str(), "/repo/deps/external");
        assert_eq!(lore_args.pin.as_str(), "v2.0");
    }

    #[test]
    fn update_result_serializes() {
        let result = UpdateResult {
            link_path: "deps/external".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("deps/external"));
    }

    #[test]
    fn update_result_deserializes() {
        let json = r#"{"link_path":"deps/external"}"#;
        let result: UpdateResult = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(result.link_path, "deps/external");
    }
}
