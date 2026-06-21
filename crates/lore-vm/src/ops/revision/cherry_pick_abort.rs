//! `revision cherry_pick_abort` operation — binds `lore::revision::cherry_pick_abort`.
//!
//! Aborts an in-progress cherry-pick and restores the working directory to its
//! prior state. Takes no arguments beyond the repository context.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::revision::LoreRevisionCherryPickAbortArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`cherry_pick_abort`].
///
/// The upstream `LoreRevisionCherryPickAbortArgs` is an empty struct —
/// only the repository context (provided by `LoreApi`) is needed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CherryPickAbortArgs {}

impl CherryPickAbortArgs {
    fn into_lore(self) -> LoreRevisionCherryPickAbortArgs {
        LoreRevisionCherryPickAbortArgs {}
    }
}

/// Result returned on successful cherry-pick abort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CherryPickAbortResult {
    /// Whether the abort completed successfully.
    pub aborted: bool,
}

/// Abort an in-progress cherry-pick operation.
///
/// Calls the upstream `lore::revision::cherry_pick_abort` in-process and checks
/// the completion status. Returns success when the cherry-pick is fully aborted.
pub async fn cherry_pick_abort(
    api: &LoreApi,
    args: CherryPickAbortArgs,
) -> Result<CherryPickAbortResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::revision::cherry_pick_abort(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("cherry_pick_abort failed with status {status}"),
        )));
    }

    Ok(CherryPickAbortResult { aborted: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises() {
        let args = CherryPickAbortArgs {};
        let json = serde_json::to_string(&args).expect("should serialise");
        assert_eq!(json, "{}");
    }

    #[test]
    fn args_deserialises() {
        let json = "{}";
        let _args: CherryPickAbortArgs = serde_json::from_str(json).expect("should deserialise");
    }

    #[test]
    fn args_into_lore_conversion() {
        let args = CherryPickAbortArgs {};
        let _lore_args = args.into_lore();
    }

    #[test]
    fn result_serialises() {
        let result = CherryPickAbortResult { aborted: true };
        let json = serde_json::to_string(&result).expect("should serialise");
        assert!(json.contains("true"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"aborted": true}"#;
        let result: CherryPickAbortResult = serde_json::from_str(json).expect("should deserialise");
        assert!(result.aborted);
    }

    #[test]
    fn args_default() {
        let args = CherryPickAbortArgs::default();
        let json = serde_json::to_string(&args).expect("should serialise");
        assert_eq!(json, "{}");
    }
}
