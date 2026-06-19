//! `file dirty_move` operation — binds `lore::file::dirty_move`.
//!
//! Relocates a staged node to a new path, flagging the destination as
//! `DirtyMove` and removing the source. No filesystem checks are
//! performed — this is a metadata-only staging operation.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileDirtyMoveArgs;
use lore::interface::LoreString;
use serde::{Deserialize, Serialize};

/// Arguments for [`dirty_move`].
///
/// Mirrors `LoreFileDirtyMoveArgs` from the upstream `lore` crate but uses
/// plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDirtyMoveArgs {
    /// Original path of the file to move.
    pub from_path: String,
    /// New destination path.
    pub to_path: String,
}

impl FileDirtyMoveArgs {
    fn into_lore(self) -> LoreFileDirtyMoveArgs {
        LoreFileDirtyMoveArgs {
            from_path: LoreString::from_str(&self.from_path),
            to_path: LoreString::from_str(&self.to_path),
        }
    }
}

/// Result returned on a successful dirty move.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDirtyMoveResult {
    /// Original path that was moved from.
    pub from_path: String,
    /// Destination path that was created.
    pub to_path: String,
}

/// Mark a file as dirty-moved in the staging area.
///
/// Calls the upstream `lore::file::dirty_move` in-process. The operation
/// relocates the staged node to the new path and flags it `DirtyMove`; no
/// filesystem I/O occurs. Emits only standard `Complete`/`Error` events.
pub async fn dirty_move(api: &LoreApi, args: FileDirtyMoveArgs) -> Result<FileDirtyMoveResult> {
    let from = args.from_path.clone();
    let to = args.to_path.clone();

    let (callback, rx) = collect_events();

    let status = lore::file::dirty_move(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file dirty_move failed with status {status}"),
        )));
    }

    Ok(FileDirtyMoveResult {
        from_path: from,
        to_path: to,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_move_args_serializes() {
        let args = FileDirtyMoveArgs {
            from_path: "src/main.rs".into(),
            to_path: "src/app.rs".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("src/app.rs"));
    }

    #[test]
    fn dirty_move_args_deserializes() {
        let json = r#"{"from_path":"a.txt","to_path":"b.txt"}"#;
        let args: FileDirtyMoveArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.from_path, "a.txt");
        assert_eq!(args.to_path, "b.txt");
    }

    #[test]
    fn dirty_move_args_into_lore_conversion() {
        let args = FileDirtyMoveArgs {
            from_path: "hello.md".into(),
            to_path: "world.md".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.from_path.as_str(), "hello.md");
        assert_eq!(lore_args.to_path.as_str(), "world.md");
    }

    #[test]
    fn dirty_move_result_serializes() {
        let result = FileDirtyMoveResult {
            from_path: "src/lib.rs".into(),
            to_path: "src/core.rs".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("src/lib.rs"));
        assert!(json.contains("src/core.rs"));
    }

    #[test]
    fn dirty_move_result_round_trip() {
        let result = FileDirtyMoveResult {
            from_path: "assets/texture.png".into(),
            to_path: "assets/renamed_texture.png".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: FileDirtyMoveResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.from_path, result.from_path);
        assert_eq!(deserialized.to_path, result.to_path);
    }
}
