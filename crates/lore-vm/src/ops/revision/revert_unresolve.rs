//! `revision revert_unresolve` operation — binds `lore::revision::revert_unresolve`.
//!
//! Marks the specified resolved paths as unresolved again during an in-progress
//! revert, allowing the user to re-resolve them.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreString;
use lore::revision::LoreRevisionRevertUnresolveArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`revert_unresolve`].
///
/// Mirrors `LoreRevisionRevertUnresolveArgs` from the upstream `lore` crate
/// but uses plain `Vec<String>` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertUnresolveArgs {
    /// Repository-relative paths to mark as unresolved.
    pub paths: Vec<String>,
}

impl RevertUnresolveArgs {
    fn into_lore(self) -> LoreRevisionRevertUnresolveArgs {
        LoreRevisionRevertUnresolveArgs {
            paths: lore::interface::LoreArray::from_vec(
                self.paths
                    .into_iter()
                    .map(|p| LoreString::from_str(&p))
                    .collect(),
            ),
        }
    }
}

/// Result returned on successful revert unresolve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertUnresolveResult {
    /// The paths that were marked as unresolved.
    pub paths: Vec<String>,
}

/// Mark revert conflicts on the given paths as unresolved.
///
/// Calls the upstream `lore::revision::revert_unresolve` in-process and
/// returns a typed result echoing the paths that were unresolved.
pub async fn revert_unresolve(
    api: &LoreApi,
    args: RevertUnresolveArgs,
) -> Result<RevertUnresolveResult> {
    let paths = args.paths.clone();

    let (callback, rx) = collect_events();

    let status =
        lore::revision::revert_unresolve(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revert_unresolve failed with status {status}"),
        )));
    }

    Ok(RevertUnresolveResult { paths })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises() {
        let args = RevertUnresolveArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialise");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"paths":["a.txt","b.txt"]}"#;
        let args: RevertUnresolveArgs = serde_json::from_str(json).expect("should deserialise");
        assert_eq!(args.paths, vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn result_serialises() {
        let result = RevertUnresolveResult {
            paths: vec!["file.txt".into()],
        };
        let json = serde_json::to_string(&result).expect("should serialise");
        assert!(json.contains("file.txt"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"paths":["x.rs"]}"#;
        let result: RevertUnresolveResult = serde_json::from_str(json).expect("should deserialise");
        assert_eq!(result.paths, vec!["x.rs"]);
    }

    #[test]
    fn args_empty_paths() {
        let args = RevertUnresolveArgs { paths: vec![] };
        let json = serde_json::to_string(&args).expect("should serialise");
        let round: RevertUnresolveArgs = serde_json::from_str(&json).expect("should deserialise");
        assert!(round.paths.is_empty());
    }

    #[test]
    fn into_lore_converts() {
        let args = RevertUnresolveArgs {
            paths: vec!["a.txt".into()],
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.paths.len(), 1);
    }
}
