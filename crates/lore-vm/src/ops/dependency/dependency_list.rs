//! `dependency dependency_list` operation — binds `lore::dependency::dependency_list`.
//!
//! Lists file dependencies (or dependents) at a given revision.
//! Calls [`lore::dependency::dependency_list`] in-process (no CLI shelling) and
//! collects `FileDependencyList*` events to return typed results.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::dependency::LoreFileDependencyListArgs;
use lore::interface::{LoreArray, LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`dependency_list`].
///
/// Mirrors `LoreFileDependencyListArgs` from the upstream `lore` crate
/// but uses plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyListArgs {
    /// File paths to query dependencies for.
    pub paths: Vec<String>,
    /// Revision to query at (empty string for current).
    #[serde(default)]
    pub revision: String,
    /// Follow transitive dependencies recursively.
    #[serde(default)]
    pub recursive: bool,
    /// Return dependents (reverse lookup) instead of dependencies.
    #[serde(default)]
    pub reverse: bool,
    /// Filter results by tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Maximum recursion depth (0 = unlimited).
    #[serde(default)]
    pub depth_limit: u32,
}

impl DependencyListArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreFileDependencyListArgs {
        LoreFileDependencyListArgs {
            paths: LoreArray::from_vec(
                self.paths
                    .into_iter()
                    .map(|p| {
                        let path = std::path::Path::new(&p);
                        if path.is_absolute() {
                            LoreString::from_str(&p)
                        } else {
                            LoreString::from_path(repo_root.join(path))
                        }
                    })
                    .collect(),
            ),
            revision: LoreString::from_str(&self.revision),
            recursive: u8::from(self.recursive),
            reverse: u8::from(self.reverse),
            tags: LoreArray::from_vec(
                self.tags
                    .into_iter()
                    .map(|s| LoreString::from_str(&s))
                    .collect(),
            ),
            depth_limit: self.depth_limit,
        }
    }
}

/// A single dependency entry for a queried file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEntry {
    /// Path of the dependency (or dependent in reverse mode).
    pub path: String,
    /// Tags on this dependency edge.
    pub tags: Vec<String>,
    /// Traversal depth — 0 for a direct dependency.
    pub depth: u32,
}

/// Dependencies for a single queried file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDependencies {
    /// The queried file path.
    pub path: String,
    /// Dependency entries for this file.
    pub entries: Vec<DependencyEntry>,
}

/// Result of a successful `dependency list` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyListResult {
    /// Number of queried files.
    pub file_count: u64,
    /// Per-file dependency listings.
    pub files: Vec<FileDependencies>,
    /// Total number of dependency entries across all files.
    pub total_entry_count: u64,
}

/// List file dependencies (or dependents) at a given revision.
///
/// Calls upstream `lore::dependency::dependency_list` in-process, collects the
/// `FileDependencyList*` events, and returns a typed result.
pub async fn dependency_list(
    api: &LoreApi,
    args: DependencyListArgs,
) -> Result<DependencyListResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::dependency::dependency_list(globals.build(), args.into_lore(&repo_root), callback)
            .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("dependency_list failed with status {status}"),
        )));
    }

    // Parse the event stream into structured results.
    // Events arrive in order: ListBegin, then per file: ListFile, ListEntry*, ListFileEnd,
    // finally ListEnd.
    let mut files: Vec<FileDependencies> = Vec::new();
    let mut current_file: Option<FileDependencies> = None;
    let mut total_entry_count: u64 = 0;
    let mut file_count: u64 = 0;

    for event in &stream.events {
        match event {
            LoreEvent::FileDependencyListBegin(data) => {
                file_count = data.file_count;
            }
            LoreEvent::FileDependencyListFile(data) => {
                // Start a new file group.
                if let Some(prev) = current_file.take() {
                    files.push(prev);
                }
                current_file = Some(FileDependencies {
                    path: data.path.as_str().to_string(),
                    entries: Vec::with_capacity(data.entry_count as usize),
                });
            }
            LoreEvent::FileDependencyListEntry(data) => {
                if let Some(ref mut file) = current_file {
                    file.entries.push(DependencyEntry {
                        path: data.path.as_str().to_string(),
                        tags: data
                            .tags
                            .as_slice()
                            .iter()
                            .map(|t| t.as_str().to_string())
                            .collect(),
                        depth: data.depth,
                    });
                }
            }
            LoreEvent::FileDependencyListFileEnd(_) => {
                if let Some(file) = current_file.take() {
                    files.push(file);
                }
            }
            LoreEvent::FileDependencyListEnd(data) => {
                total_entry_count = data.total_entry_count;
            }
            _ => {}
        }
    }

    // Flush any remaining file if ListFileEnd was missing.
    if let Some(file) = current_file.take() {
        files.push(file);
    }

    Ok(DependencyListResult {
        file_count,
        files,
        total_entry_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = DependencyListResult {
            file_count: 1,
            files: vec![FileDependencies {
                path: "assets/hero.fbx".into(),
                entries: vec![DependencyEntry {
                    path: "textures/hero_diffuse.png".into(),
                    tags: vec!["texture".into()],
                    depth: 0,
                }],
            }],
            total_entry_count: 1,
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"file_count\":1"));
        assert!(json.contains("\"path\":\"assets/hero.fbx\""));
        assert!(json.contains("\"path\":\"textures/hero_diffuse.png\""));
        assert!(json.contains("\"tags\":[\"texture\"]"));
        assert!(json.contains("\"depth\":0"));
        assert!(json.contains("\"total_entry_count\":1"));
    }

    #[test]
    fn empty_list_result() {
        let result = DependencyListResult {
            file_count: 0,
            files: vec![],
            total_entry_count: 0,
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"file_count\":0"));
        assert!(json.contains("\"files\":[]"));
        assert!(json.contains("\"total_entry_count\":0"));
    }

    #[test]
    fn args_default_fields() {
        let args: DependencyListArgs = serde_json::from_str(r#"{"paths":["a.txt"]}"#).unwrap();
        assert_eq!(args.paths, vec!["a.txt"]);
        assert_eq!(args.revision, "");
        assert!(!args.recursive);
        assert!(!args.reverse);
        assert!(args.tags.is_empty());
        assert_eq!(args.depth_limit, 0);
    }

    #[test]
    fn args_converts_to_lore() {
        let args = DependencyListArgs {
            paths: vec!["a.txt".into(), "b.txt".into()],
            revision: "main".into(),
            recursive: true,
            reverse: false,
            tags: vec!["texture".into()],
            depth_limit: 3,
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.paths.as_slice().len(), 2);
        assert_eq!(lore_args.revision.as_str(), "main");
        assert_eq!(lore_args.recursive, 1);
        assert_eq!(lore_args.reverse, 0);
        assert_eq!(lore_args.tags.as_slice().len(), 1);
        assert_eq!(lore_args.depth_limit, 3);
    }

    #[test]
    fn multiple_files_with_entries() {
        let result = DependencyListResult {
            file_count: 2,
            files: vec![
                FileDependencies {
                    path: "a.fbx".into(),
                    entries: vec![
                        DependencyEntry {
                            path: "tex_a.png".into(),
                            tags: vec![],
                            depth: 0,
                        },
                        DependencyEntry {
                            path: "tex_b.png".into(),
                            tags: vec!["diffuse".into(), "normal".into()],
                            depth: 1,
                        },
                    ],
                },
                FileDependencies {
                    path: "b.fbx".into(),
                    entries: vec![],
                },
            ],
            total_entry_count: 2,
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"file_count\":2"));
        assert!(json.contains("\"total_entry_count\":2"));
        // Second file has empty entries
        assert!(json.contains("\"path\":\"b.fbx\""));
    }
}
