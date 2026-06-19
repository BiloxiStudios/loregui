//! `dependency dependency_add` operation — binds `lore::dependency::dependency_add`.
//!
//! Adds file dependencies to the current repository. Each entry in `sources`
//! is a `(source_path, dependencies)` pair where `dependencies` is a slice of
//! `(dependency_path, tags)`. Tags classify the dependency edge (e.g. "texture",
//! "compile"). Cycle detection is performed unless `force` is set.
//!
//! Corresponding back-references on target files are created automatically.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::dependency::LoreFileDependencyAddArgs;
use lore::interface::LoreArray;
use lore::interface::LoreString;
use serde::{Deserialize, Serialize};

/// A single dependency entry to add.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyAddEntry {
    /// Path of the dependency target file.
    pub dependency: String,
    /// Tags to apply to this dependency edge (e.g. "texture", "compile").
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A source file with dependencies to add.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyAddSource {
    /// Path of the source file.
    pub path: String,
    /// Dependencies to add to this source.
    pub dependencies: Vec<DependencyAddEntry>,
}

/// Arguments for [`dependency_add`].
///
/// Provides a more ergonomic, Rust-idiomatic interface over the raw
/// parallel-array structure used by the upstream C API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyAddArgs {
    /// Source files with their dependencies to add.
    pub sources: Vec<DependencyAddSource>,
    /// Skip cycle detection when true.
    #[serde(default)]
    pub force: bool,
}

impl DependencyAddArgs {
    fn into_lore(self) -> LoreFileDependencyAddArgs {
        let mut paths = Vec::new();
        let mut dependencies = Vec::new();
        let mut tags = Vec::new();
        let mut dep_counts = Vec::new();
        let mut tag_counts = Vec::new();

        for source in &self.sources {
            paths.push(LoreString::from_str(&source.path));
            dep_counts.push(source.dependencies.len() as u32);

            for entry in &source.dependencies {
                dependencies.push(LoreString::from_str(&entry.dependency));
                tag_counts.push(entry.tags.len() as u32);

                for tag in &entry.tags {
                    tags.push(LoreString::from_str(tag));
                }
            }
        }

        LoreFileDependencyAddArgs {
            paths: LoreArray::from_vec(paths),
            dependencies: LoreArray::from_vec(dependencies),
            tags: LoreArray::from_vec(tags),
            dep_counts: LoreArray::from_vec(dep_counts),
            tag_counts: LoreArray::from_vec(tag_counts),
            force: u8::from(self.force),
        }
    }
}

/// Result returned on successful dependency addition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyAddResult {
    /// Number of dependency edges that were added.
    pub added_count: u64,
}

/// Add file dependencies to the current repository.
///
/// Calls the upstream `lore::dependency::dependency_add` in-process and
/// collects the `FileDependencyAddEnd` event to return a typed result.
pub async fn dependency_add(
    api: &LoreApi,
    args: DependencyAddArgs,
) -> Result<DependencyAddResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::dependency::dependency_add(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("dependency_add failed with status {status}"),
        )));
    }

    let added_count = stream.dependency_add_end().ok_or_else(|| {
        LoreError::Parse("dependency_add succeeded but no FileDependencyAddEnd event emitted".into())
    })?;

    Ok(DependencyAddResult { added_count })
}

// Extension trait for EventStream to extract dependency_add results.
trait DependencyAddExt {
    fn dependency_add_end(&self) -> Option<u64>;
}

impl DependencyAddExt for crate::collect::EventStream {
    fn dependency_add_end(&self) -> Option<u64> {
        use lore::interface::LoreEvent;

        for event in &self.events {
            if let LoreEvent::FileDependencyAddEnd(data) = event {
                return Some(data.added_count);
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
        let args = DependencyAddArgs {
            sources: vec![DependencyAddSource {
                path: "/foo/bar.txt".into(),
                dependencies: vec![DependencyAddEntry {
                    dependency: "/baz/qux.txt".into(),
                    tags: vec!["compile".into()],
                }],
            }],
            force: false,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("foo/bar.txt"));
        assert!(json.contains("baz/qux.txt"));
        assert!(json.contains("compile"));
    }

    #[test]
    fn args_into_lore_empty_tags() {
        let args = DependencyAddArgs {
            sources: vec![DependencyAddSource {
                path: "/foo.txt".into(),
                dependencies: vec![DependencyAddEntry {
                    dependency: "/bar.txt".into(),
                    tags: vec![],
                }],
            }],
            force: false,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.paths.len(), 1);
        assert_eq!(lore_args.dependencies.len(), 1);
        assert_eq!(lore_args.dep_counts.len(), 1);
        assert_eq!(lore_args.dep_counts.as_slice()[0], 1);
        assert_eq!(lore_args.tag_counts.len(), 1);
        assert_eq!(lore_args.tag_counts.as_slice()[0], 0);
        assert_eq!(lore_args.force, 0);
    }

    #[test]
    fn args_into_lore_with_force() {
        let args = DependencyAddArgs {
            sources: vec![DependencyAddSource {
                path: "/foo.txt".into(),
                dependencies: vec![DependencyAddEntry {
                    dependency: "/bar.txt".into(),
                    tags: vec![],
                }],
            }],
            force: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.force, 1);
    }

    #[test]
    fn args_into_lore_multiple_sources() {
        let args = DependencyAddArgs {
            sources: vec![
                DependencyAddSource {
                    path: "/a.txt".into(),
                    dependencies: vec![
                        DependencyAddEntry {
                            dependency: "/b.txt".into(),
                            tags: vec!["tag1".into()],
                        },
                        DependencyAddEntry {
                            dependency: "/c.txt".into(),
                            tags: vec!["tag2".into(), "tag3".into()],
                        },
                    ],
                },
                DependencyAddSource {
                    path: "/d.txt".into(),
                    dependencies: vec![],
                },
            ],
            force: false,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.paths.len(), 2);
        assert_eq!(lore_args.dependencies.len(), 2);
        assert_eq!(lore_args.tags.len(), 3);
        assert_eq!(lore_args.dep_counts.as_slice(), &[2, 0]);
        assert_eq!(lore_args.tag_counts.as_slice(), &[1, 2]);
    }

    #[test]
    fn result_serializes() {
        let result = DependencyAddResult {
            added_count: 42,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("42"));
    }

    #[test]
    fn args_deserializes_defaults() {
        let args: DependencyAddArgs = serde_json::from_str(
            r#"{"sources":[{"path":"a.txt","dependencies":[{"dependency":"b.txt"}]}]}"#,
        )
        .unwrap();
        assert!(!args.force);
        assert!(args.sources[0].dependencies[0].tags.is_empty());
    }
}
