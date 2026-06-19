//! `file write` operation — binds `lore::file::write`.
//!
//! Writes file content from the repository to a destination filesystem path.
//! Can resolve files by path+revision or by direct content address.
//! Emits `FileWrite` with the destination path on success.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileWriteArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`write`].
///
/// Mirrors `LoreFileWriteArgs` from the upstream `lore` crate but uses plain
/// `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteArgs {
    /// Content address to write; takes precedence over `path` when non-empty.
    #[serde(default)]
    pub address: String,
    /// Repository-relative path to the file (used when `address` is empty).
    #[serde(default)]
    pub path: String,
    /// Revision of the file to write (used with `path`).
    #[serde(default)]
    pub revision: String,
    /// Destination filesystem path to write to.
    pub output: String,
}

impl FileWriteArgs {
    fn into_lore(self) -> LoreFileWriteArgs {
        LoreFileWriteArgs {
            address: LoreString::from_str(&self.address),
            path: LoreString::from_str(&self.path),
            revision: LoreString::from_str(&self.revision),
            output: LoreString::from_str(&self.output),
        }
    }
}

/// Result returned on a successful file write.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteResult {
    /// Destination path the file was written to.
    pub path: String,
}

/// Write file content from the repository to a destination path.
///
/// Calls the upstream `lore::file::write` in-process and collects
/// `FileWrite` events into a typed result.
pub async fn write(api: &LoreApi, args: FileWriteArgs) -> Result<FileWriteResult> {
    let (callback, rx) = collect_events();

    let status = lore::file::write(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file write failed with status {status}"),
        )));
    }

    let mut written_path = String::new();

    for event in &stream.events {
        if let LoreEvent::FileWrite(data) = event {
            written_path = data.path.as_str().to_string();
        }
    }

    Ok(FileWriteResult { path: written_path })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_args_serializes() {
        let args = FileWriteArgs {
            address: String::new(),
            path: "src/main.rs".into(),
            revision: "abc123".into(),
            output: "/tmp/out.rs".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("abc123"));
        assert!(json.contains("/tmp/out.rs"));
    }

    #[test]
    fn write_args_deserializes_with_defaults() {
        let json = r#"{"output": "/tmp/file.bin"}"#;
        let args: FileWriteArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(args.address.is_empty());
        assert!(args.path.is_empty());
        assert!(args.revision.is_empty());
        assert_eq!(args.output, "/tmp/file.bin");
    }

    #[test]
    fn write_args_with_address() {
        let args = FileWriteArgs {
            address: "addr-abc".into(),
            path: String::new(),
            revision: String::new(),
            output: "/tmp/out.bin".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.address.as_str(), "addr-abc");
        assert!(lore_args.path.as_str().is_empty());
        assert_eq!(lore_args.output.as_str(), "/tmp/out.bin");
    }

    #[test]
    fn write_args_with_path_and_revision() {
        let args = FileWriteArgs {
            address: String::new(),
            path: "assets/texture.png".into(),
            revision: "rev42".into(),
            output: "/tmp/texture.png".into(),
        };
        let lore_args = args.into_lore();
        assert!(lore_args.address.as_str().is_empty());
        assert_eq!(lore_args.path.as_str(), "assets/texture.png");
        assert_eq!(lore_args.revision.as_str(), "rev42");
        assert_eq!(lore_args.output.as_str(), "/tmp/texture.png");
    }

    #[test]
    fn write_result_serializes() {
        let result = FileWriteResult {
            path: "/tmp/written.bin".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("/tmp/written.bin"));
    }

    #[test]
    fn write_result_round_trip() {
        let result = FileWriteResult {
            path: "/home/user/output.txt".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: FileWriteResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.path, result.path);
    }
}
