//! `file dirty_copy` operation — binds `lore::file::dirty_copy`.
//!
//! Creates a new staged destination node flagged as `DirtyCopy`; the source
//! node is unchanged. No filesystem checks are performed — this is a
//! metadata-only staging operation.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileDirtyCopyArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`dirty_copy`].
///
/// Mirrors `LoreFileDirtyCopyArgs` from the upstream `lore` crate but uses
/// plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDirtyCopyArgs {
    /// Source path of the file to copy.
    pub from_path: String,
    /// Destination path for the copy.
    pub to_path: String,
}

impl FileDirtyCopyArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreFileDirtyCopyArgs {
        LoreFileDirtyCopyArgs {
            from_path: crate::ops::paths::lore_path_arg(repo_root, &self.from_path),
            to_path: crate::ops::paths::lore_path_arg(repo_root, &self.to_path),
        }
    }
}

/// Result returned on a successful dirty copy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDirtyCopyResult {
    /// Source path that was copied from.
    pub from_path: String,
    /// Destination path that was created.
    pub to_path: String,
}

/// Mark a file as dirty-copied in the staging area.
///
/// Calls the upstream `lore::file::dirty_copy` in-process. The operation
/// creates a new destination node flagged `DirtyCopy`; no filesystem I/O
/// occurs. Emits only standard `Complete`/`Error` events.
pub async fn dirty_copy(api: &LoreApi, args: FileDirtyCopyArgs) -> Result<FileDirtyCopyResult> {
    let from = args.from_path.clone();
    let to = args.to_path.clone();

    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::file::dirty_copy(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file dirty_copy failed with status {status}"),
        )));
    }

    Ok(FileDirtyCopyResult {
        from_path: from,
        to_path: to,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_copy_args_serializes() {
        let args = FileDirtyCopyArgs {
            from_path: "src/main.rs".into(),
            to_path: "src/main_copy.rs".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("src/main_copy.rs"));
    }

    #[test]
    fn dirty_copy_args_deserializes() {
        let json = r#"{"from_path":"a.txt","to_path":"b.txt"}"#;
        let args: FileDirtyCopyArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.from_path, "a.txt");
        assert_eq!(args.to_path, "b.txt");
    }

    #[test]
    fn dirty_copy_args_into_lore_conversion() {
        let args = FileDirtyCopyArgs {
            from_path: "hello.md".into(),
            to_path: "hello_copy.md".into(),
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.from_path.as_str(), "/repo/hello.md");
        assert_eq!(lore_args.to_path.as_str(), "/repo/hello_copy.md");
    }

    #[test]
    fn dirty_copy_result_serializes() {
        let result = FileDirtyCopyResult {
            from_path: "src/lib.rs".into(),
            to_path: "src/lib_backup.rs".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("src/lib.rs"));
        assert!(json.contains("src/lib_backup.rs"));
    }

    #[test]
    fn dirty_copy_result_round_trip() {
        let result = FileDirtyCopyResult {
            from_path: "assets/texture.png".into(),
            to_path: "assets/texture_v2.png".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: FileDirtyCopyResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.from_path, result.from_path);
        assert_eq!(deserialized.to_path, result.to_path);
    }
}
