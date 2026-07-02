//! `dependency dependency_remove` operation — binds `lore::dependency::dependency_remove`.
//!
//! Removes file dependencies from the current repository. Each entry in `sources`
//! is a `(source_path, dependencies)` pair where `dependencies` is a slice of
//! `(dependency_path, tags)`. If `tags` is empty for a dependency, the entire
//! dependency edge is removed. If tags are specified, only those tags are removed
//! and the edge is removed entirely when no tags remain.
//!
//! Corresponding back-references on target files are updated automatically.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::dependency::LoreFileDependencyRemoveArgs;
use lore::interface::LoreArray;
use lore::interface::LoreString;
use serde::{Deserialize, Serialize};

/// A single dependency entry to remove.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyRemoveEntry {
    /// Path of the dependency target file.
    pub dependency: String,
    /// Tags to remove from this dependency edge.
    /// If empty, the entire dependency is removed.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A source file with dependencies to remove.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyRemoveSource {
    /// Path of the source file.
    pub path: String,
    /// Dependencies to remove from this source.
    pub dependencies: Vec<DependencyRemoveEntry>,
}

/// Arguments for [`dependency_remove`].
///
/// Provides a more ergonomic, Rust-idiomatic interface over the raw
/// parallel-array structure used by the upstream C API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyRemoveArgs {
    /// Source files with their dependencies to remove.
    pub sources: Vec<DependencyRemoveSource>,
}

impl DependencyRemoveArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreFileDependencyRemoveArgs {
        let mut paths = Vec::new();
        let mut dependencies = Vec::new();
        let mut tags = Vec::new();
        let mut dep_counts = Vec::new();
        let mut tag_counts = Vec::new();

        for source in &self.sources {
            paths.push(crate::ops::paths::lore_path_arg(repo_root, &source.path));
            dep_counts.push(source.dependencies.len() as u32);

            for entry in &source.dependencies {
                dependencies.push(crate::ops::paths::lore_path_arg(
                    repo_root,
                    &entry.dependency,
                ));
                tag_counts.push(entry.tags.len() as u32);

                for tag in &entry.tags {
                    tags.push(LoreString::from_str(tag));
                }
            }
        }

        LoreFileDependencyRemoveArgs {
            paths: LoreArray::from_vec(paths),
            dependencies: LoreArray::from_vec(dependencies),
            tags: LoreArray::from_vec(tags),
            dep_counts: LoreArray::from_vec(dep_counts),
            tag_counts: LoreArray::from_vec(tag_counts),
        }
    }
}

/// Result returned on successful dependency removal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyRemoveResult {
    /// Number of dependency edges that were removed.
    pub removed_count: u64,
}

/// Remove file dependencies from the current repository.
///
/// Calls the upstream `lore::dependency::dependency_remove` in-process and
/// collects the `FileDependencyRemoveEnd` event to return a typed result.
pub async fn dependency_remove(
    api: &LoreApi,
    args: DependencyRemoveArgs,
) -> Result<DependencyRemoveResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::dependency::dependency_remove(globals.build(), args.into_lore(&repo_root), callback)
            .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("dependency_remove failed with status {status}"),
        )));
    }

    let removed_count = stream.dependency_remove_end().ok_or_else(|| {
        LoreError::Parse(
            "dependency_remove succeeded but no FileDependencyRemoveEnd event emitted".into(),
        )
    })?;

    Ok(DependencyRemoveResult { removed_count })
}

// Extension trait for EventStream to extract dependency_remove results.
trait DependencyRemoveExt {
    fn dependency_remove_end(&self) -> Option<u64>;
}

impl DependencyRemoveExt for crate::collect::EventStream {
    fn dependency_remove_end(&self) -> Option<u64> {
        use lore::interface::LoreEvent;

        for event in &self.events {
            if let LoreEvent::FileDependencyRemoveEnd(data) = event {
                return Some(data.removed_count);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes() {
        let args = DependencyRemoveArgs {
            sources: vec![DependencyRemoveSource {
                path: "/foo/bar.txt".into(),
                dependencies: vec![DependencyRemoveEntry {
                    dependency: "/baz/qux.txt".into(),
                    tags: vec!["compile".into()],
                }],
            }],
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("foo/bar.txt"));
        assert!(json.contains("baz/qux.txt"));
        assert!(json.contains("compile"));
    }

    #[test]
    fn args_into_lore_empty_tags() {
        let args = DependencyRemoveArgs {
            sources: vec![DependencyRemoveSource {
                path: "/foo.txt".into(),
                dependencies: vec![DependencyRemoveEntry {
                    dependency: "/bar.txt".into(),
                    tags: vec![],
                }],
            }],
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.paths.len(), 1);
        assert_eq!(lore_args.dependencies.len(), 1);
        assert_eq!(lore_args.dep_counts.len(), 1);
        assert_eq!(lore_args.dep_counts.as_slice()[0], 1);
        assert_eq!(lore_args.tag_counts.len(), 1);
        assert_eq!(lore_args.tag_counts.as_slice()[0], 0);
    }

    #[test]
    fn args_into_lore_multiple_sources() {
        let args = DependencyRemoveArgs {
            sources: vec![
                DependencyRemoveSource {
                    path: "/a.txt".into(),
                    dependencies: vec![
                        DependencyRemoveEntry {
                            dependency: "/b.txt".into(),
                            tags: vec!["tag1".into()],
                        },
                        DependencyRemoveEntry {
                            dependency: "/c.txt".into(),
                            tags: vec!["tag2".into(), "tag3".into()],
                        },
                    ],
                },
                DependencyRemoveSource {
                    path: "/d.txt".into(),
                    dependencies: vec![],
                },
            ],
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.paths.len(), 2);
        assert_eq!(lore_args.dependencies.len(), 2);
        assert_eq!(lore_args.tags.len(), 3);
        assert_eq!(lore_args.dep_counts.as_slice(), &[2, 0]);
        assert_eq!(lore_args.tag_counts.as_slice(), &[1, 2]);
    }

    #[test]
    fn result_serializes() {
        let result = DependencyRemoveResult { removed_count: 42 };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("42"));
    }
}
