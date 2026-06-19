//! `repository verify_fragment` operation — binds `lore::repository::verify_fragment`.
//!
//! Verifies a single fragment in the local (or remote) immutable store by its
//! hash and optional context. Emits `RepositoryVerifyFragment` (local) or
//! `RepositoryVerifyFragmentRemote` events with match details.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::repository::LoreRepositoryVerifyFragmentArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`verify_fragment`].
///
/// Mirrors `LoreRepositoryVerifyFragmentArgs` from the upstream `lore` crate
/// but uses plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyFragmentArgs {
    /// Fragment hash to verify (hex string).
    pub hash: String,
    /// Optional context to match; empty string matches any.
    #[serde(default)]
    pub context: String,
    /// Heal detected inconsistencies during verification.
    #[serde(default)]
    pub heal: bool,
}

impl VerifyFragmentArgs {
    fn into_lore(self) -> LoreRepositoryVerifyFragmentArgs {
        LoreRepositoryVerifyFragmentArgs {
            hash: LoreString::from_str(&self.hash),
            context: LoreString::from_str(&self.context),
            heal: if self.heal { 1 } else { 0 },
        }
    }
}

/// One stored copy of a fragment found during local verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentMatch {
    pub slot: u32,
    pub index: u32,
    pub repository: String,
    pub address_hash: String,
    pub address_context: String,
    pub flags: u32,
    pub size_payload: u32,
    pub size_content: u64,
    pub pack_offset: u32,
    pub pack_file: u32,
    pub last_access: u64,
}

/// Result of a local fragment verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyFragmentLocalResult {
    pub hash: String,
    pub group_index: u32,
    pub bucket_index: u32,
    pub index_path: String,
    pub entry_count: u32,
    pub packfile_entry_count: u32,
    pub match_count: u32,
    pub matches: Vec<FragmentMatch>,
    /// Non-empty when the fragment has a verification error.
    pub error: String,
}

/// Result of a remote fragment verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyFragmentRemoteResult {
    pub address_hash: String,
    pub address_context: String,
    pub corrupted: bool,
    pub healed: bool,
    /// Non-empty when the remote verification produced an error.
    pub error: String,
}

/// Combined result of `verify_fragment` — either local or remote depending on
/// the repository's global args.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum VerifyFragmentResult {
    Local(VerifyFragmentLocalResult),
    Remote(VerifyFragmentRemoteResult),
}

/// Verify a single fragment in the immutable store.
///
/// Calls the upstream `lore::repository::verify_fragment` in-process and
/// collects the resulting event to return a typed result.
pub async fn verify_fragment(
    api: &LoreApi,
    args: VerifyFragmentArgs,
) -> Result<VerifyFragmentResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::repository::verify_fragment(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("verify_fragment failed with status {status}"),
        )));
    }

    for event in &stream.events {
        if let LoreEvent::RepositoryVerifyFragment(data) = event {
            let matches = data
                .matches
                .as_slice()
                .iter()
                .map(|m| FragmentMatch {
                    slot: m.slot,
                    index: m.index,
                    repository: format!("{}", m.repository),
                    address_hash: format!("{}", m.address_hash),
                    address_context: format!("{}", m.address_context),
                    flags: m.flags,
                    size_payload: m.size_payload,
                    size_content: m.size_content,
                    pack_offset: m.pack_offset,
                    pack_file: m.pack_file,
                    last_access: m.last_access,
                })
                .collect();

            return Ok(VerifyFragmentResult::Local(VerifyFragmentLocalResult {
                hash: format!("{}", data.hash),
                group_index: data.group_index,
                bucket_index: data.bucket_index,
                index_path: data.index_path.as_str().to_string(),
                entry_count: data.entry_count,
                packfile_entry_count: data.packfile_entry_count,
                match_count: data.match_count,
                matches,
                error: data.error.as_str().to_string(),
            }));
        }

        if let LoreEvent::RepositoryVerifyFragmentRemote(data) = event {
            return Ok(VerifyFragmentResult::Remote(VerifyFragmentRemoteResult {
                address_hash: format!("{}", data.address_hash),
                address_context: format!("{}", data.address_context),
                corrupted: data.corrupted != 0,
                healed: data.healed != 0,
                error: data.error.as_str().to_string(),
            }));
        }
    }

    Err(LoreError::Parse(
        "verify_fragment completed but no verification event emitted".into(),
    ))
}
