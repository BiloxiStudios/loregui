//! `revision cherry_pick_restart` operation — binds `lore::revision::cherry_pick_restart`.
//!
//! Re-materialises the specified paths for resolution during an in-progress
//! cherry-pick conflict.  This discards any partial resolution work on those
//! paths and puts them back to the conflicted state so the user can start over.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreString;
use lore::revision::LoreRevisionCherryPickRestartArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`cherry_pick_restart`].
///
/// Mirrors `LoreRevisionCherryPickRestartArgs` from the upstream `lore` crate
/// but uses plain `Vec<String>` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CherryPickRestartArgs {
    /// Repository-relative paths to re-materialise for resolution.
    pub paths: Vec<String>,
}

impl CherryPickRestartArgs {
    fn into_lore(self) -> LoreRevisionCherryPickRestartArgs {
        LoreRevisionCherryPickRestartArgs {
            paths: lore::interface::LoreArray::from_vec(
                self.paths
                    .into_iter()
                    .map(|p| LoreString::from_str(&p))
                    .collect(),
            ),
        }
    }
}

/// Result returned on successful cherry-pick restart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CherryPickRestartResult {
    /// The paths that were re-materialised for resolution.
    pub paths: Vec<String>,
}

/// Re-materialise paths for resolution during an in-progress cherry-pick conflict.
///
/// Calls the upstream `lore::revision::cherry_pick_restart` in-process and
/// returns a typed result echoing the paths that were restarted.
pub async fn cherry_pick_restart(
    api: &LoreApi,
    args: CherryPickRestartArgs,
) -> Result<CherryPickRestartResult> {
    let paths = args.paths.clone();

    let (callback, rx) = collect_events();

    let status =
        lore::revision::cherry_pick_restart(api.globals().build(), args.into_lore(), callback)
            .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("cherry_pick_restart failed with status {status}"),
        )));
    }

    Ok(CherryPickRestartResult { paths })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises() {
        let args = CherryPickRestartArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialise");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"paths":["a.txt","b.txt"]}"#;
        let args: CherryPickRestartArgs = serde_json::from_str(json).expect("should deserialise");
        assert_eq!(args.paths, vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn result_serialises() {
        let result = CherryPickRestartResult {
            paths: vec!["file.txt".into()],
        };
        let json = serde_json::to_string(&result).expect("should serialise");
        assert!(json.contains("file.txt"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"paths":["x.rs"]}"#;
        let result: CherryPickRestartResult =
            serde_json::from_str(json).expect("should deserialise");
        assert_eq!(result.paths, vec!["x.rs"]);
    }

    #[test]
    fn args_empty_paths() {
        let args = CherryPickRestartArgs { paths: vec![] };
        let json = serde_json::to_string(&args).expect("should serialise");
        let round: CherryPickRestartArgs = serde_json::from_str(&json).expect("should deserialise");
        assert!(round.paths.is_empty());
    }

    #[test]
    fn into_lore_converts() {
        let args = CherryPickRestartArgs {
            paths: vec!["a.txt".into()],
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.paths.len(), 1);
    }
}
