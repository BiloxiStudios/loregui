//! `lock file_acquire` operation — binds `lore::lock::file_acquire`.
//!
//! Acquires exclusive locks on one or more files in the repository.
//! Emits `LockFileAcquire` events for each successfully acquired lock,
//! and `LockFileAcquireIgnore` events for each file already owned by the user.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::lock::LoreLockFileAcquireArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`file_acquire`].
///
/// Mirrors `LoreLockFileAcquireArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAcquireArgs {
    /// Paths to acquire locks on.
    pub paths: Vec<String>,
    /// Branch the locks are acquired on.
    pub branch: String,
}

impl FileAcquireArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreLockFileAcquireArgs {
        LoreLockFileAcquireArgs {
            paths: crate::ops::paths::lore_path_args(repo_root, &self.paths),
            branch: LoreString::from_str(&self.branch),
        }
    }
}

/// Result returned on successful file lock acquisition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAcquireResult {
    /// Paths for which locks were successfully acquired.
    pub acquired: Vec<String>,
    /// Paths that were skipped because locks were already owned.
    pub ignored: Vec<String>,
}

/// Acquires file locks on the specified paths for a given branch.
///
/// Calls the upstream `lore::lock::file_acquire` in-process and collects
/// the `LockFileAcquire` and `LockFileAcquireIgnore` events to return
/// a typed result.
pub async fn file_acquire(api: &LoreApi, args: FileAcquireArgs) -> Result<FileAcquireResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::lock::file_acquire(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file_acquire failed with status {status}"),
        )));
    }

    let mut acquired = Vec::new();
    let mut ignored = Vec::new();

    for event in &stream.events {
        match event {
            LoreEvent::LockFileAcquire(data) => {
                acquired.push(data.path.as_str().to_string());
            }
            LoreEvent::LockFileAcquireIgnore(data) => {
                ignored.push(data.path.as_str().to_string());
            }
            _ => {}
        }
    }

    Ok(FileAcquireResult { acquired, ignored })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes() {
        let args = FileAcquireArgs {
            paths: vec!["src/main.rs".into()],
            branch: "dev".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("dev"));
    }

    #[test]
    fn args_deserializes() {
        let json = r#"{"paths":["a.rs"],"branch":"main"}"#;
        let args: FileAcquireArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.paths, vec!["a.rs"]);
        assert_eq!(args.branch, "main");
    }

    #[test]
    fn into_lore_relative_path_joined() {
        let args = FileAcquireArgs {
            paths: vec!["assets/mesh.uasset".into()],
            branch: "main".into(),
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo/root"));
        assert_eq!(
            lore_args.paths.as_slice()[0].as_str(),
            "/repo/root/assets/mesh.uasset"
        );
    }

    #[test]
    fn into_lore_empty_path_preserved() {
        let args = FileAcquireArgs {
            paths: vec![String::new()],
            branch: "main".into(),
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo/root"));
        assert_eq!(
            lore_args.paths.as_slice()[0].as_str(),
            "",
            "empty path must stay empty (no-filter sentinel)"
        );
    }

    #[test]
    fn into_lore_absolute_path_preserved() {
        let args = FileAcquireArgs {
            paths: vec!["/abs/path.rs".into()],
            branch: "main".into(),
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo/root"));
        assert_eq!(lore_args.paths.as_slice()[0].as_str(), "/abs/path.rs");
    }

    #[test]
    fn into_lore_branch_unchanged() {
        let args = FileAcquireArgs {
            paths: vec!["file.rs".into()],
            branch: "feature/test".into(),
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.branch.as_str(), "feature/test");
    }

    #[test]
    fn result_serializes() {
        let result = FileAcquireResult {
            acquired: vec!["src/main.rs".into()],
            ignored: vec!["src/lib.rs".into()],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("acquired"));
        assert!(json.contains("ignored"));
    }
}
