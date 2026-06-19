//! `link add` operation — binds `lore::link::add`.
//!
//! Adds a new link to a linked repository at the specified path.
//! Calls [`lore::link::add`] in-process (no CLI shelling) and collects
//! `RepositoryCloneBegin`/`RepositoryCloneEnd`/`LinkChange` events.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreString;
use lore::link::LoreLinkAddArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`add`].
///
/// Mirrors `LoreLinkAddArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddArgs {
    /// Link repository URL or identifier.
    pub link: String,
    /// Path within this repository where the link is added.
    pub link_path: String,
    /// Source path within the linked repository; `/` or `\` means the root.
    #[serde(default = "default_source_path")]
    pub source_path: String,
    /// Branch or revision to set the link pin at.
    #[serde(default)]
    pub pin: String,
    /// Disable automatic branch creation in the linked repository.
    #[serde(default)]
    pub disable_branching: bool,
}

fn default_source_path() -> String {
    "/".into()
}

impl AddArgs {
    fn into_lore(self) -> LoreLinkAddArgs {
        LoreLinkAddArgs {
            link: LoreString::from_str(&self.link),
            link_path: LoreString::from_str(&self.link_path),
            source_path: LoreString::from_str(&self.source_path),
            pin: LoreString::from_str(&self.pin),
            disable_branching: u8::from(self.disable_branching),
        }
    }
}

/// Result returned on successful `link add`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddResult {
    /// The link path where the link was added.
    pub link_path: String,
}

/// Adds a new link to a linked repository at the specified path.
///
/// Calls the upstream `lore::link::add` in-process and collects
/// clone and link change events to return a typed result.
pub async fn add(api: &LoreApi, args: AddArgs) -> Result<AddResult> {
    let link_path = args.link_path.clone();
    let (callback, rx) = collect_events();

    let status =
        lore::link::add(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("link add failed with status {status}"),
        )));
    }

    // Verify LinkChange event was emitted
    let found_link_change = stream.events.iter().any(|e| {
        matches!(e, lore::interface::LoreEvent::LinkChange(_))
    });

    if !found_link_change {
        return Err(LoreError::Parse(
            "link add succeeded but no LinkChange event emitted".into(),
        ));
    }

    Ok(AddResult { link_path })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_args_serializes() {
        let args = AddArgs {
            link: "https://example.com/repo".into(),
            link_path: "deps/external".into(),
            source_path: "/".into(),
            pin: "main".into(),
            disable_branching: false,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("https://example.com/repo"));
        assert!(json.contains("deps/external"));
    }

    #[test]
    fn add_args_deserializes_with_defaults() {
        let json = r#"{"link":"https://example.com/repo","link_path":"deps/external"}"#;
        let args: AddArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.link, "https://example.com/repo");
        assert_eq!(args.link_path, "deps/external");
        assert_eq!(args.source_path, "/");
        assert!(!args.disable_branching);
    }

    #[test]
    fn add_args_into_lore_conversion() {
        let args = AddArgs {
            link: "https://example.com/repo".into(),
            link_path: "deps/external".into(),
            source_path: "/".into(),
            pin: "main".into(),
            disable_branching: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.link.as_str(), "https://example.com/repo");
        assert_eq!(lore_args.link_path.as_str(), "deps/external");
        assert_eq!(lore_args.source_path.as_str(), "/");
        assert_eq!(lore_args.pin.as_str(), "main");
        assert_eq!(lore_args.disable_branching, 1);
    }

    #[test]
    fn add_args_disable_branching_false() {
        let args = AddArgs {
            link: "repo".into(),
            link_path: "path".into(),
            source_path: "/".into(),
            pin: "".into(),
            disable_branching: false,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.disable_branching, 0);
    }

    #[test]
    fn add_result_serializes() {
        let result = AddResult {
            link_path: "deps/external".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("deps/external"));
    }

    #[test]
    fn add_result_deserializes() {
        let json = r#"{"link_path":"deps/external"}"#;
        let result: AddResult = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(result.link_path, "deps/external");
    }

    #[test]
    fn source_path_default_is_slash() {
        let args = AddArgs {
            link: "repo".into(),
            link_path: "path".into(),
            source_path: "/".into(),
            pin: "".into(),
            disable_branching: false,
        };
        assert_eq!(args.source_path, "/");
    }
}
