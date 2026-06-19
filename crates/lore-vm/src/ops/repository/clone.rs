//! `repository clone` operation — binds `lore::repository::clone`.
//!
//! Clones a remote repository to the local path specified in the global
//! arguments. Emits `RepositoryCloneBegin`, `RepositoryCloneProgress`, and
//! `RepositoryCloneEnd` events during the operation.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreArray, LoreEvent, LoreString};
use lore::repository::LoreRepositoryCloneArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`clone`].
///
/// Mirrors `LoreRepositoryCloneArgs` from the upstream `lore` crate but uses
/// plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CloneArgs {
    /// URL to the repository (e.g. `lore://host/repo`).
    pub repository_url: String,
    /// Optional revision to clone; empty string clones the latest.
    #[serde(default)]
    pub revision: String,
    /// Optional client-side view filter to use.
    #[serde(default)]
    pub view: String,
    /// Clone without any files (bare clone).
    #[serde(default)]
    pub bare: bool,
    /// Clone virtually using split-write filesystem.
    #[serde(default)]
    pub virtually: bool,
    /// Use direct file write.
    #[serde(default)]
    pub direct_file_write: bool,
    /// Use direct file I/O instead of memory mapping files.
    #[serde(default)]
    pub direct_file_io: bool,
    /// Optional layer module.
    #[serde(default)]
    pub layer: String,
    /// Optional layer metadata key to link revisions with.
    #[serde(default)]
    pub layer_metadata: String,
    /// Optional file containing list of files to prefetch.
    #[serde(default)]
    pub prefetch: String,
    /// Use the shared store instead of a local immutable store.
    #[serde(default)]
    pub use_shared_store: bool,
    /// Optional path for the shared store; empty string uses the default.
    #[serde(default)]
    pub shared_store_path: String,
    /// Clone without local repository tracking (memory-only stores).
    #[serde(default)]
    pub no_tracking: bool,
    /// Root files for dependency-based selective clone.
    #[serde(default)]
    pub root_files: Vec<String>,
    /// Tags to filter dependencies by during resolution.
    #[serde(default)]
    pub dependency_tags: Vec<String>,
    /// Follow transitive dependencies recursively.
    #[serde(default)]
    pub dependency_recursive: bool,
    /// Maximum dependency traversal depth. 0 means unlimited.
    #[serde(default)]
    pub dependency_depth_limit: u32,
}

impl CloneArgs {
    fn into_lore(self) -> LoreRepositoryCloneArgs {
        let lore_root_files: Vec<LoreString> = self
            .root_files
            .iter()
            .map(|s| LoreString::from_str(s))
            .collect();
        let lore_dep_tags: Vec<LoreString> = self
            .dependency_tags
            .iter()
            .map(|s| LoreString::from_str(s))
            .collect();

        LoreRepositoryCloneArgs {
            repository_url: LoreString::from_str(&self.repository_url),
            revision: LoreString::from_str(&self.revision),
            view: LoreString::from_str(&self.view),
            bare: u8::from(self.bare),
            virtually: u8::from(self.virtually),
            direct_file_write: u8::from(self.direct_file_write),
            direct_file_io: u8::from(self.direct_file_io),
            layer: LoreString::from_str(&self.layer),
            layer_metadata: LoreString::from_str(&self.layer_metadata),
            prefetch: LoreString::from_str(&self.prefetch),
            use_shared_store: u8::from(self.use_shared_store),
            shared_store_path: LoreString::from_str(&self.shared_store_path),
            no_tracking: u8::from(self.no_tracking),
            root_files: LoreArray::from_vec(lore_root_files),
            dependency_tags: LoreArray::from_vec(lore_dep_tags),
            dependency_recursive: u8::from(self.dependency_recursive),
            dependency_depth_limit: self.dependency_depth_limit,
        }
    }
}

/// Progress counts reported during a clone operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CloneProgress {
    /// Number of files finished.
    pub file_complete: u64,
    /// Number of files retained (already matched).
    pub file_retain: u64,
    /// Number of files replaced.
    pub file_replace: u64,
    /// Total number of files discovered.
    pub file_count: u64,
    /// Number of files currently being processed.
    pub file_inflight: u64,
    /// Number of fragment fetches in flight.
    pub fragment_inflight: u64,
    /// Bytes transferred so far.
    pub bytes_transferred: u64,
    /// Total bytes to transfer.
    pub bytes_total: u64,
    /// Whether file discovery has completed.
    pub discovery_complete: bool,
}

/// Result of a successful `clone` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneResult {
    /// Repository identifier.
    pub repository: String,
    /// Branch name that was cloned.
    pub branch: String,
    /// Revision that was cloned.
    pub revision: String,
    /// Local path the clone was written to.
    pub path: String,
    /// Final progress counts.
    pub progress: CloneProgress,
}

/// Clone a remote repository to the local working directory.
///
/// Calls the upstream `lore::repository::clone` in-process and collects the
/// `RepositoryCloneBegin` and `RepositoryCloneEnd` events to return a typed
/// result.
pub async fn clone(api: &LoreApi, args: CloneArgs) -> Result<CloneResult> {
    let (callback, rx) = collect_events();

    let status = lore::repository::clone(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("repository clone failed with status {status}"),
        )));
    }

    // Extract begin data (repository id + path) and end data (branch + revision + counts).
    let mut repository = String::new();
    let mut path = String::new();
    let mut branch = String::new();
    let mut revision = String::new();
    let mut progress = CloneProgress::default();

    for event in &stream.events {
        match event {
            LoreEvent::RepositoryCloneBegin(data) => {
                repository = format!("{}", data.repository);
                path = data.path.as_str().to_string();
            }
            LoreEvent::RepositoryCloneEnd(data) => {
                branch = data.branch.as_str().to_string();
                revision = format!("{}", data.revision);
                progress = CloneProgress {
                    file_complete: data.count.file_complete,
                    file_retain: data.count.file_retain,
                    file_replace: data.count.file_replace,
                    file_count: data.count.file_count,
                    file_inflight: data.count.file_inflight,
                    fragment_inflight: data.count.fragment_inflight,
                    bytes_transferred: data.count.bytes_transferred,
                    bytes_total: data.count.bytes_total,
                    discovery_complete: data.count.discovery_complete != 0,
                };
            }
            _ => {}
        }
    }

    if repository.is_empty() {
        return Err(LoreError::Parse(
            "clone completed but no RepositoryCloneBegin event emitted".into(),
        ));
    }

    Ok(CloneResult {
        repository,
        branch,
        revision,
        path,
        progress,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_args_defaults() {
        let json = r#"{"repository_url":"lore://host/repo"}"#;
        let args: CloneArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.repository_url, "lore://host/repo");
        assert_eq!(args.revision, "");
        assert!(!args.bare);
        assert!(!args.use_shared_store);
        assert!(args.root_files.is_empty());
        assert_eq!(args.dependency_depth_limit, 0);
    }

    #[test]
    fn clone_args_full_roundtrip() {
        let args = CloneArgs {
            repository_url: "lore://host/repo".into(),
            revision: "abc123".into(),
            view: "view1".into(),
            bare: true,
            virtually: false,
            direct_file_write: true,
            direct_file_io: false,
            layer: String::new(),
            layer_metadata: String::new(),
            prefetch: String::new(),
            use_shared_store: true,
            shared_store_path: "/tmp/store".into(),
            no_tracking: false,
            root_files: vec!["a.txt".into(), "b.txt".into()],
            dependency_tags: vec!["tag1".into()],
            dependency_recursive: true,
            dependency_depth_limit: 5,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        let back: CloneArgs = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(back.repository_url, "lore://host/repo");
        assert_eq!(back.revision, "abc123");
        assert!(back.bare);
        assert!(back.use_shared_store);
        assert_eq!(back.root_files.len(), 2);
        assert_eq!(back.dependency_depth_limit, 5);
    }

    #[test]
    fn clone_args_into_lore_conversion() {
        let args = CloneArgs {
            repository_url: "lore://host/repo".into(),
            revision: "rev1".into(),
            bare: true,
            use_shared_store: true,
            shared_store_path: "/tmp/store".into(),
            root_files: vec!["file.txt".into()],
            dependency_tags: vec!["tag1".into(), "tag2".into()],
            dependency_recursive: true,
            dependency_depth_limit: 3,
            ..Default::default()
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.repository_url.as_str(), "lore://host/repo");
        assert_eq!(lore_args.revision.as_str(), "rev1");
        assert_eq!(lore_args.bare, 1);
        assert_eq!(lore_args.use_shared_store, 1);
        assert_eq!(lore_args.shared_store_path.as_str(), "/tmp/store");
        assert_eq!(lore_args.root_files.len(), 1);
        assert_eq!(lore_args.dependency_tags.len(), 2);
        assert_eq!(lore_args.dependency_recursive, 1);
        assert_eq!(lore_args.dependency_depth_limit, 3);
    }

    #[test]
    fn clone_progress_defaults() {
        let p = CloneProgress::default();
        assert_eq!(p.file_complete, 0);
        assert_eq!(p.bytes_total, 0);
        assert!(!p.discovery_complete);
    }

    #[test]
    fn clone_result_serializes() {
        let result = CloneResult {
            repository: "repo-id".into(),
            branch: "main".into(),
            revision: "abc123".into(),
            path: "/tmp/clone".into(),
            progress: CloneProgress {
                file_complete: 10,
                file_retain: 2,
                file_replace: 8,
                file_count: 10,
                file_inflight: 0,
                fragment_inflight: 0,
                bytes_transferred: 1024,
                bytes_total: 1024,
                discovery_complete: true,
            },
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("repo-id"));
        assert!(json.contains("main"));
        assert!(json.contains("abc123"));
        assert!(json.contains("1024"));
    }
}
