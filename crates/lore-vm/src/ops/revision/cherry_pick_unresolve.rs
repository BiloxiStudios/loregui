//! `revision cherry_pick_unresolve` operation — binds `lore::revision::cherry_pick_unresolve`.
//!
//! Marks the specified resolved paths as unresolved again during an in-progress
//! cherry-pick, allowing the user to re-resolve them.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreString;
use lore::revision::LoreRevisionCherryPickUnresolveArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`cherry_pick_unresolve`].
///
/// Mirrors `LoreRevisionCherryPickUnresolveArgs` from the upstream `lore` crate
/// but uses plain `Vec<String>` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CherryPickUnresolveArgs {
    /// Repository-relative paths to mark as unresolved.
    pub paths: Vec<String>,
}

impl CherryPickUnresolveArgs {
    fn into_lore(self) -> LoreRevisionCherryPickUnresolveArgs {
        LoreRevisionCherryPickUnresolveArgs {
            paths: lore::interface::LoreArray::from_vec(
                self.paths
                    .into_iter()
                    .map(|p| LoreString::from_str(&p))
                    .collect(),
            ),
        }
    }
}

/// Result returned on successful cherry-pick unresolve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CherryPickUnresolveResult {
    /// The paths that were marked as unresolved.
    pub paths: Vec<String>,
}

/// Mark cherry-pick conflicts on the given paths as unresolved.
///
/// Calls the upstream `lore::revision::cherry_pick_unresolve` in-process and
/// returns a typed result echoing the paths that were unresolved.
pub async fn cherry_pick_unresolve(
    api: &LoreApi,
    args: CherryPickUnresolveArgs,
) -> Result<CherryPickUnresolveResult> {
    let paths = args.paths.clone();

    let (callback, rx) = collect_events();

    let status =
        lore::revision::cherry_pick_unresolve(api.globals().build(), args.into_lore(), callback)
            .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("cherry_pick_unresolve failed with status {status}"),
        )));
    }

    Ok(CherryPickUnresolveResult { paths })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises() {
        let args = CherryPickUnresolveArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialise");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"paths":["a.txt","b.txt"]}"#;
        let args: CherryPickUnresolveArgs = serde_json::from_str(json).expect("should deserialise");
        assert_eq!(args.paths, vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn result_serialises() {
        let result = CherryPickUnresolveResult {
            paths: vec!["file.txt".into()],
        };
        let json = serde_json::to_string(&result).expect("should serialise");
        assert!(json.contains("file.txt"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"paths":["x.rs"]}"#;
        let result: CherryPickUnresolveResult =
            serde_json::from_str(json).expect("should deserialise");
        assert_eq!(result.paths, vec!["x.rs"]);
    }

    #[test]
    fn args_empty_paths() {
        let args = CherryPickUnresolveArgs { paths: vec![] };
        let json = serde_json::to_string(&args).expect("should serialise");
        let round: CherryPickUnresolveArgs =
            serde_json::from_str(&json).expect("should deserialise");
        assert!(round.paths.is_empty());
    }

    #[test]
    fn into_lore_converts() {
        let args = CherryPickUnresolveArgs {
            paths: vec!["a.txt".into()],
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.paths.len(), 1);
    }
}
