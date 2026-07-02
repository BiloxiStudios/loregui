//! `revision revert_resolve_mine` operation — binds `lore::revision::revert_resolve_mine`.
//!
//! Resolves the specified conflicted paths during an in-progress revert by
//! keeping the "mine" (local/current-branch) version of each file.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::revision::LoreRevisionRevertResolveMineArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`revert_resolve_mine`].
///
/// Mirrors `LoreRevisionRevertResolveMineArgs` from the upstream `lore` crate
/// but uses plain `Vec<String>` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertResolveMineArgs {
    /// Repository-relative paths to resolve in favor of "mine" (local).
    pub paths: Vec<String>,
}

impl RevertResolveMineArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreRevisionRevertResolveMineArgs {
        LoreRevisionRevertResolveMineArgs {
            paths: crate::ops::paths::lore_path_args(repo_root, &self.paths),
        }
    }
}

/// Result returned on successful revert resolve-mine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertResolveMineResult {
    /// The paths that were resolved in favor of "mine".
    pub paths: Vec<String>,
}

/// Resolve revert conflicts on the given paths by keeping the "mine" version.
///
/// Calls the upstream `lore::revision::revert_resolve_mine` in-process and
/// returns a typed result echoing the paths that were resolved.
pub async fn revert_resolve_mine(
    api: &LoreApi,
    args: RevertResolveMineArgs,
) -> Result<RevertResolveMineResult> {
    let paths = args.paths.clone();

    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::revision::revert_resolve_mine(globals.build(), args.into_lore(&repo_root), callback)
            .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revert_resolve_mine failed with status {status}"),
        )));
    }

    Ok(RevertResolveMineResult { paths })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises() {
        let args = RevertResolveMineArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialise");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"paths":["a.txt","b.txt"]}"#;
        let args: RevertResolveMineArgs = serde_json::from_str(json).expect("should deserialise");
        assert_eq!(args.paths, vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn result_serialises() {
        let result = RevertResolveMineResult {
            paths: vec!["file.txt".into()],
        };
        let json = serde_json::to_string(&result).expect("should serialise");
        assert!(json.contains("file.txt"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"paths":["x.rs"]}"#;
        let result: RevertResolveMineResult =
            serde_json::from_str(json).expect("should deserialise");
        assert_eq!(result.paths, vec!["x.rs"]);
    }

    #[test]
    fn args_empty_paths() {
        let args = RevertResolveMineArgs { paths: vec![] };
        let json = serde_json::to_string(&args).expect("should serialise");
        let round: RevertResolveMineArgs = serde_json::from_str(&json).expect("should deserialise");
        assert!(round.paths.is_empty());
    }

    #[test]
    fn into_lore_converts() {
        let args = RevertResolveMineArgs {
            paths: vec!["a.txt".into()],
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.paths.len(), 1);
    }
}
