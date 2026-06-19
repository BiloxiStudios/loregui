//! `repository verify_state` operation — binds `lore::repository::verify_state`.
//!
//! Verifies the integrity of the local repository state, optionally healing
//! detected inconsistencies. Collects fragment-level verification events and
//! returns a typed summary.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::interface::LoreString;
use lore::repository::LoreRepositoryVerifyStateArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`verify_state`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyStateArgs {
    /// Repository-relative path to verify; empty verifies the whole repository.
    #[serde(default)]
    pub path: String,
    /// When true, attempt to heal detected inconsistencies.
    #[serde(default)]
    pub heal: bool,
}

/// A single fragment that was verified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedFragment {
    /// Hex-encoded hash of the fragment.
    pub hash: String,
    /// Number of stored copies found.
    pub match_count: u32,
    /// Error message, if verification failed for this fragment.
    pub error: String,
}

/// A remote fragment verification result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedRemoteFragment {
    /// Hex-encoded hash part of the fragment address.
    pub address_hash: String,
    /// Whether the fragment was found corrupted.
    pub corrupted: bool,
    /// Whether the fragment was healed.
    pub healed: bool,
    /// Error message, if any.
    pub error: String,
}

/// Result of a successful `verify_state` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyStateResult {
    /// Hex-encoded hash of the healed staged state (all zeros when nothing was healed).
    pub healed_staged_state: String,
    /// Fragments verified locally.
    pub fragments: Vec<VerifiedFragment>,
    /// Fragments verified against the remote.
    pub remote_fragments: Vec<VerifiedRemoteFragment>,
    /// Number of local fragments with errors.
    pub error_count: u32,
    /// Number of corrupted remote fragments.
    pub corrupted_count: u32,
}

/// Verify the integrity of the local repository state.
///
/// Calls upstream `lore::repository::verify_state` in-process, collects
/// verification events, and returns a typed summary.
pub async fn verify_state(api: &LoreApi, args: VerifyStateArgs) -> Result<VerifyStateResult> {
    let lore_args = LoreRepositoryVerifyStateArgs {
        path: LoreString::from_str(&args.path),
        heal: u8::from(args.heal),
    };

    let (callback, rx) = collect_events();

    let status = lore::repository::verify_state(api.globals().build(), lore_args, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("verify_state failed with status {status}"),
        )));
    }

    let mut healed_staged_state = String::new();
    let mut fragments = Vec::new();
    let mut remote_fragments = Vec::new();

    for event in &stream.events {
        match event {
            LoreEvent::RepositoryVerifyStateEnd(data) => {
                healed_staged_state = format!("{}", data.healed_staged_state);
            }
            LoreEvent::RepositoryVerifyFragment(data) => {
                fragments.push(VerifiedFragment {
                    hash: format!("{}", data.hash),
                    match_count: data.match_count,
                    error: data.error.as_str().to_string(),
                });
            }
            LoreEvent::RepositoryVerifyFragmentRemote(data) => {
                remote_fragments.push(VerifiedRemoteFragment {
                    address_hash: format!("{}", data.address_hash),
                    corrupted: data.corrupted != 0,
                    healed: data.healed != 0,
                    error: data.error.as_str().to_string(),
                });
            }
            _ => {}
        }
    }

    let error_count = fragments.iter().filter(|f| !f.error.is_empty()).count() as u32;
    let corrupted_count = remote_fragments.iter().filter(|f| f.corrupted).count() as u32;

    Ok(VerifyStateResult {
        healed_staged_state,
        fragments,
        remote_fragments,
        error_count,
        corrupted_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = VerifyStateResult {
            healed_staged_state: "00".repeat(32),
            fragments: vec![VerifiedFragment {
                hash: "abc123".into(),
                match_count: 2,
                error: String::new(),
            }],
            remote_fragments: vec![],
            error_count: 0,
            corrupted_count: 0,
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"error_count\":0"));
        assert!(json.contains("\"match_count\":2"));
    }

    #[test]
    fn args_defaults() {
        let args: VerifyStateArgs = serde_json::from_str("{}").expect("deserialise");
        assert_eq!(args.path, "");
        assert!(!args.heal);
    }

    #[test]
    fn error_count_computed_correctly() {
        let result = VerifyStateResult {
            healed_staged_state: String::new(),
            fragments: vec![
                VerifiedFragment {
                    hash: "a".into(),
                    match_count: 1,
                    error: String::new(),
                },
                VerifiedFragment {
                    hash: "b".into(),
                    match_count: 0,
                    error: "missing".into(),
                },
            ],
            remote_fragments: vec![VerifiedRemoteFragment {
                address_hash: "c".into(),
                corrupted: true,
                healed: false,
                error: "corrupt".into(),
            }],
            error_count: 1,
            corrupted_count: 1,
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"error_count\":1"));
        assert!(json.contains("\"corrupted_count\":1"));
    }
}
