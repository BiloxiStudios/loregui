//! `revision revert_resolve_theirs` operation — binds `lore::revision::revert_resolve_theirs`.
//!
//! Resolves the specified conflicted paths during an in-progress revert by
//! keeping the "theirs" (reverted/incoming) version of each file.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreString;
use lore::revision::LoreRevisionRevertResolveTheirsArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`revert_resolve_theirs`].
///
/// Mirrors `LoreRevisionRevertResolveTheirsArgs` from the upstream `lore` crate
/// but uses plain `Vec<String>` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertResolveTheirsArgs {
    /// Repository-relative paths to resolve in favor of "theirs" (incoming).
    pub paths: Vec<String>,
}

impl RevertResolveTheirsArgs {
    fn into_lore(self) -> LoreRevisionRevertResolveTheirsArgs {
        LoreRevisionRevertResolveTheirsArgs {
            paths: lore::interface::LoreArray::from_vec(
                self.paths
                    .into_iter()
                    .map(|p| LoreString::from_str(&p))
                    .collect(),
            ),
        }
    }
}

/// Result returned on successful revert resolve-theirs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertResolveTheirsResult {
    /// The paths that were resolved in favor of "theirs".
    pub paths: Vec<String>,
}

/// Resolve revert conflicts on the given paths by keeping the "theirs" version.
///
/// Calls the upstream `lore::revision::revert_resolve_theirs` in-process and
/// returns a typed result echoing the paths that were resolved.
pub async fn revert_resolve_theirs(
    api: &LoreApi,
    args: RevertResolveTheirsArgs,
) -> Result<RevertResolveTheirsResult> {
    let paths = args.paths.clone();

    let (callback, rx) = collect_events();

    let status =
        lore::revision::revert_resolve_theirs(api.globals().build(), args.into_lore(), callback)
            .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revert_resolve_theirs failed with status {status}"),
        )));
    }

    Ok(RevertResolveTheirsResult { paths })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises() {
        let args = RevertResolveTheirsArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialise");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"paths":["a.txt","b.txt"]}"#;
        let args: RevertResolveTheirsArgs = serde_json::from_str(json).expect("should deserialise");
        assert_eq!(args.paths, vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn result_serialises() {
        let result = RevertResolveTheirsResult {
            paths: vec!["file.txt".into()],
        };
        let json = serde_json::to_string(&result).expect("should serialise");
        assert!(json.contains("file.txt"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"paths":["x.rs"]}"#;
        let result: RevertResolveTheirsResult =
            serde_json::from_str(json).expect("should deserialise");
        assert_eq!(result.paths, vec!["x.rs"]);
    }

    #[test]
    fn args_empty_paths() {
        let args = RevertResolveTheirsArgs { paths: vec![] };
        let json = serde_json::to_string(&args).expect("should serialise");
        let round: RevertResolveTheirsArgs =
            serde_json::from_str(&json).expect("should deserialise");
        assert!(round.paths.is_empty());
    }

    #[test]
    fn into_lore_converts() {
        let args = RevertResolveTheirsArgs {
            paths: vec!["a.txt".into()],
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.paths.len(), 1);
    }
}
