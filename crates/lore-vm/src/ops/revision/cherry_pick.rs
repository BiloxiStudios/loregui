//! `revision cherry_pick` operation — binds `lore::revision::cherry_pick`.
//!
//! Cherry-picks a revision onto the current branch with remote dispatch.
//! Uses the same arguments as `cherry_pick_local` but routes through
//! lore's `dispatch_call` for server-side execution when connected.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreString;
use lore::revision::LoreRevisionCherryPickArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`cherry_pick`].
///
/// Mirrors `LoreRevisionCherryPickArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CherryPickArgs {
    /// Revision identifier to cherry-pick (hash, tag, branch name, etc.).
    pub revision: String,
    /// Commit message used for the auto-commit when no conflicts arise.
    #[serde(default)]
    pub message: String,
    /// When `true`, skip auto-commit even if no conflicts arise.
    #[serde(default)]
    pub no_commit: bool,
}

impl CherryPickArgs {
    fn into_lore(self) -> LoreRevisionCherryPickArgs {
        LoreRevisionCherryPickArgs {
            revision: LoreString::from_str(&self.revision),
            message: LoreString::from_str(&self.message),
            no_commit: u8::from(self.no_commit),
        }
    }
}

/// Result returned on successful cherry-pick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CherryPickResult {
    /// The revision that was cherry-picked.
    pub revision: String,
}

/// Cherry-pick a revision onto the current branch (with remote dispatch).
///
/// Calls the upstream `lore::revision::cherry_pick` in-process and
/// returns a typed result echoing the revision that was applied.
pub async fn cherry_pick(api: &LoreApi, args: CherryPickArgs) -> Result<CherryPickResult> {
    let revision = args.revision.clone();

    let (callback, rx) = collect_events();

    let status =
        lore::revision::cherry_pick(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("cherry_pick failed with status {status}"),
        )));
    }

    Ok(CherryPickResult { revision })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises() {
        let args = CherryPickArgs {
            revision: "abc123".into(),
            message: "cherry-pick commit".into(),
            no_commit: false,
        };
        let json = serde_json::to_string(&args).expect("should serialise");
        assert!(json.contains("abc123"));
        assert!(json.contains("cherry-pick commit"));
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"revision":"abc123","message":"pick it","no_commit":true}"#;
        let args: CherryPickArgs = serde_json::from_str(json).expect("should deserialise");
        assert_eq!(args.revision, "abc123");
        assert_eq!(args.message, "pick it");
        assert!(args.no_commit);
    }

    #[test]
    fn args_defaults() {
        let json = r#"{"revision":"def456"}"#;
        let args: CherryPickArgs = serde_json::from_str(json).expect("should deserialise");
        assert_eq!(args.revision, "def456");
        assert_eq!(args.message, "");
        assert!(!args.no_commit);
    }

    #[test]
    fn result_serialises() {
        let result = CherryPickResult {
            revision: "abc123".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialise");
        assert!(json.contains("abc123"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"revision":"abc123"}"#;
        let result: CherryPickResult = serde_json::from_str(json).expect("should deserialise");
        assert_eq!(result.revision, "abc123");
    }

    #[test]
    fn into_lore_converts() {
        let args = CherryPickArgs {
            revision: "HEAD~1".into(),
            message: "test".into(),
            no_commit: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.revision.as_str(), "HEAD~1");
        assert_eq!(lore_args.message.as_str(), "test");
        assert_eq!(lore_args.no_commit, 1);
    }

    #[test]
    fn into_lore_no_commit_false() {
        let args = CherryPickArgs {
            revision: "main".into(),
            message: "".into(),
            no_commit: false,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.no_commit, 0);
    }
}
