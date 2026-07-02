//! `revision revert_resolve` operation — binds `lore::revision::revert_resolve`.
//!
//! Marks the specified conflicted paths as resolved during an in-progress
//! revert. Unlike `revert_resolve_mine` / `revert_resolve_theirs`, this
//! variant marks paths as resolved without choosing a side — the user is
//! expected to have manually edited the working-tree copies first.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::revision::LoreRevisionRevertResolveArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`revert_resolve`].
///
/// Mirrors `LoreRevisionRevertResolveArgs` from the upstream `lore` crate
/// but uses plain `Vec<String>` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertResolveArgs {
    /// Repository-relative paths to mark as resolved.
    pub paths: Vec<String>,
}

impl RevertResolveArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreRevisionRevertResolveArgs {
        LoreRevisionRevertResolveArgs {
            paths: crate::ops::paths::lore_path_args(repo_root, &self.paths),
        }
    }
}

/// Result returned on successful revert resolve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertResolveResult {
    /// The paths that were marked as resolved.
    pub paths: Vec<String>,
}

/// Mark conflicted paths as resolved during an in-progress revert.
///
/// Calls the upstream `lore::revision::revert_resolve` in-process and
/// returns a typed result echoing the paths that were resolved.
pub async fn revert_resolve(api: &LoreApi, args: RevertResolveArgs) -> Result<RevertResolveResult> {
    let paths = args.paths.clone();

    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::revision::revert_resolve(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revert_resolve failed with status {status}"),
        )));
    }

    Ok(RevertResolveResult { paths })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises() {
        let args = RevertResolveArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialise");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"paths":["a.txt","b.txt"]}"#;
        let args: RevertResolveArgs = serde_json::from_str(json).expect("should deserialise");
        assert_eq!(args.paths, vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn result_serialises() {
        let result = RevertResolveResult {
            paths: vec!["file.txt".into()],
        };
        let json = serde_json::to_string(&result).expect("should serialise");
        assert!(json.contains("file.txt"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"paths":["x.rs"]}"#;
        let result: RevertResolveResult = serde_json::from_str(json).expect("should deserialise");
        assert_eq!(result.paths, vec!["x.rs"]);
    }

    #[test]
    fn args_empty_paths() {
        let args = RevertResolveArgs { paths: vec![] };
        let json = serde_json::to_string(&args).expect("should serialise");
        let round: RevertResolveArgs = serde_json::from_str(&json).expect("should deserialise");
        assert!(round.paths.is_empty());
    }

    #[test]
    fn into_lore_converts() {
        let args = RevertResolveArgs {
            paths: vec!["a.txt".into()],
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.paths.len(), 1);
    }
}
