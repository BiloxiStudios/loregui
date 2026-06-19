//! `link remove` operation — binds `lore::link::remove`.
//!
//! Removes a link from the repository at the specified path.
//! Calls [`lore::link::remove`] in-process (no CLI shelling) and collects
//! `LinkChange` events to confirm the removal.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreString;
use lore::link::LoreLinkRemoveArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`remove`].
///
/// Mirrors `LoreLinkRemoveArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveArgs {
    /// Path within this repository where the link is removed.
    pub link_path: String,
}

impl RemoveArgs {
    fn into_lore(self) -> LoreLinkRemoveArgs {
        LoreLinkRemoveArgs {
            link_path: LoreString::from_str(&self.link_path),
        }
    }
}

/// Result returned on successful `link remove`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveResult {
    /// The link path that was removed.
    pub link_path: String,
}

/// Removes a link from the repository at the specified path.
///
/// Calls the upstream `lore::link::remove` in-process and collects
/// events to confirm the link was removed.
pub async fn remove(api: &LoreApi, args: RemoveArgs) -> Result<RemoveResult> {
    let link_path = args.link_path.clone();
    let (callback, rx) = collect_events();

    let status = lore::link::remove(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("link remove failed with status {status}"),
        )));
    }

    let found_link_change = stream
        .events
        .iter()
        .any(|e| matches!(e, lore::interface::LoreEvent::LinkChange(_)));

    if !found_link_change {
        return Err(LoreError::Parse(
            "link remove succeeded but no LinkChange event emitted".into(),
        ));
    }

    Ok(RemoveResult { link_path })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_args_serializes() {
        let args = RemoveArgs {
            link_path: "deps/external".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("deps/external"));
    }

    #[test]
    fn remove_args_deserializes() {
        let json = r#"{"link_path":"deps/external"}"#;
        let args: RemoveArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.link_path, "deps/external");
    }

    #[test]
    fn remove_args_into_lore_conversion() {
        let args = RemoveArgs {
            link_path: "deps/external".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.link_path.as_str(), "deps/external");
    }

    #[test]
    fn remove_result_serializes() {
        let result = RemoveResult {
            link_path: "deps/external".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("deps/external"));
    }

    #[test]
    fn remove_result_deserializes() {
        let json = r#"{"link_path":"deps/external"}"#;
        let result: RemoveResult = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(result.link_path, "deps/external");
    }
}
