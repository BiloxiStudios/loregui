//! `revision cherry_pick_resolve_mine` operation — binds `lore::revision::cherry_pick_resolve_mine`.
//!
//! Resolves the specified conflicted paths during an in-progress cherry-pick by
//! keeping the "mine" (local/current-branch) version of each file.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreString;
use lore::revision::LoreRevisionCherryPickResolveMineArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`cherry_pick_resolve_mine`].
///
/// Mirrors `LoreRevisionCherryPickResolveMineArgs` from the upstream `lore` crate
/// but uses plain `Vec<String>` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CherryPickResolveMineArgs {
    /// Repository-relative paths to resolve in favor of "mine" (local).
    pub paths: Vec<String>,
}

impl CherryPickResolveMineArgs {
    fn into_lore(self) -> LoreRevisionCherryPickResolveMineArgs {
        LoreRevisionCherryPickResolveMineArgs {
            paths: lore::interface::LoreArray::from_vec(
                self.paths
                    .into_iter()
                    .map(|p| LoreString::from_str(&p))
                    .collect(),
            ),
        }
    }
}

/// Result returned on successful cherry-pick resolve-mine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CherryPickResolveMineResult {
    /// The paths that were resolved in favor of "mine".
    pub paths: Vec<String>,
}

/// Resolve cherry-pick conflicts on the given paths by keeping the "mine" version.
///
/// Calls the upstream `lore::revision::cherry_pick_resolve_mine` in-process and
/// returns a typed result echoing the paths that were resolved.
pub async fn cherry_pick_resolve_mine(
    api: &LoreApi,
    args: CherryPickResolveMineArgs,
) -> Result<CherryPickResolveMineResult> {
    let paths = args.paths.clone();

    let (callback, rx) = collect_events();

    let status =
        lore::revision::cherry_pick_resolve_mine(api.globals().build(), args.into_lore(), callback)
            .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("cherry_pick_resolve_mine failed with status {status}"),
        )));
    }

    Ok(CherryPickResolveMineResult { paths })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises() {
        let args = CherryPickResolveMineArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialise");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"paths":["a.txt","b.txt"]}"#;
        let args: CherryPickResolveMineArgs =
            serde_json::from_str(json).expect("should deserialise");
        assert_eq!(args.paths, vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn result_serialises() {
        let result = CherryPickResolveMineResult {
            paths: vec!["file.txt".into()],
        };
        let json = serde_json::to_string(&result).expect("should serialise");
        assert!(json.contains("file.txt"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"paths":["x.rs"]}"#;
        let result: CherryPickResolveMineResult =
            serde_json::from_str(json).expect("should deserialise");
        assert_eq!(result.paths, vec!["x.rs"]);
    }

    #[test]
    fn args_empty_paths() {
        let args = CherryPickResolveMineArgs { paths: vec![] };
        let json = serde_json::to_string(&args).expect("should serialise");
        let round: CherryPickResolveMineArgs =
            serde_json::from_str(&json).expect("should deserialise");
        assert!(round.paths.is_empty());
    }

    #[test]
    fn into_lore_converts() {
        let args = CherryPickResolveMineArgs {
            paths: vec!["a.txt".into()],
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.paths.len(), 1);
    }
}
