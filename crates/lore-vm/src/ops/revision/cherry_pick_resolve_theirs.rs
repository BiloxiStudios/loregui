//! `revision cherry_pick_resolve_theirs` operation — binds `lore::revision::cherry_pick_resolve_theirs`.
//!
//! Resolves the specified conflicted paths during an in-progress cherry-pick by
//! keeping the "theirs" (incoming/cherry-picked) version of each file.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreString;
use lore::revision::LoreRevisionCherryPickResolveTheirsArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`cherry_pick_resolve_theirs`].
///
/// Mirrors `LoreRevisionCherryPickResolveTheirsArgs` from the upstream `lore` crate
/// but uses plain `Vec<String>` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CherryPickResolveTheirsArgs {
    /// Repository-relative paths to resolve in favor of "theirs" (incoming).
    pub paths: Vec<String>,
}

impl CherryPickResolveTheirsArgs {
    fn into_lore(self) -> LoreRevisionCherryPickResolveTheirsArgs {
        LoreRevisionCherryPickResolveTheirsArgs {
            paths: lore::interface::LoreArray::from_vec(
                self.paths
                    .into_iter()
                    .map(|p| LoreString::from_str(&p))
                    .collect(),
            ),
        }
    }
}

/// Result returned on successful cherry-pick resolve-theirs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CherryPickResolveTheirsResult {
    /// The paths that were resolved in favor of "theirs".
    pub paths: Vec<String>,
}

/// Resolve cherry-pick conflicts on the given paths by keeping the "theirs" version.
///
/// Calls the upstream `lore::revision::cherry_pick_resolve_theirs` in-process and
/// returns a typed result echoing the paths that were resolved.
pub async fn cherry_pick_resolve_theirs(
    api: &LoreApi,
    args: CherryPickResolveTheirsArgs,
) -> Result<CherryPickResolveTheirsResult> {
    let paths = args.paths.clone();

    let (callback, rx) = collect_events();

    let status = lore::revision::cherry_pick_resolve_theirs(
        api.globals().build(),
        args.into_lore(),
        callback,
    )
    .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("cherry_pick_resolve_theirs failed with status {status}"),
        )));
    }

    Ok(CherryPickResolveTheirsResult { paths })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises() {
        let args = CherryPickResolveTheirsArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialise");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"paths":["a.txt","b.txt"]}"#;
        let args: CherryPickResolveTheirsArgs =
            serde_json::from_str(json).expect("should deserialise");
        assert_eq!(args.paths, vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn result_serialises() {
        let result = CherryPickResolveTheirsResult {
            paths: vec!["file.txt".into()],
        };
        let json = serde_json::to_string(&result).expect("should serialise");
        assert!(json.contains("file.txt"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"paths":["x.rs"]}"#;
        let result: CherryPickResolveTheirsResult =
            serde_json::from_str(json).expect("should deserialise");
        assert_eq!(result.paths, vec!["x.rs"]);
    }

    #[test]
    fn args_empty_paths() {
        let args = CherryPickResolveTheirsArgs { paths: vec![] };
        let json = serde_json::to_string(&args).expect("should serialise");
        let round: CherryPickResolveTheirsArgs =
            serde_json::from_str(&json).expect("should deserialise");
        assert!(round.paths.is_empty());
    }

    #[test]
    fn into_lore_converts() {
        let args = CherryPickResolveTheirsArgs {
            paths: vec!["a.txt".into()],
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.paths.len(), 1);
    }
}
