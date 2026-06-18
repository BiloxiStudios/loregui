//! `branch metadata_set` operation — binds `lore::branch::metadata_set`.
//!
//! Sets one or more key-value metadata pairs on a branch. Use `metadata_get`
//! to read keys and `metadata_clear` to remove them.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchMetadataSetArgs;
use lore::interface::{LoreMetadataType, LoreString};
use serde::{Deserialize, Serialize};

/// Metadata value type — mirrors `LoreMetadataType` but serialises cleanly
/// across the Tauri boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetadataFormat {
    Binary,
    Numeric,
    String,
}

impl From<MetadataFormat> for LoreMetadataType {
    fn from(f: MetadataFormat) -> Self {
        match f {
            MetadataFormat::Binary => LoreMetadataType::Binary,
            MetadataFormat::Numeric => LoreMetadataType::Numeric,
            MetadataFormat::String => LoreMetadataType::String,
        }
    }
}

/// Arguments for [`metadata_set`].
///
/// Mirrors `LoreBranchMetadataSetArgs` from the upstream `lore` crate but uses
/// plain Rust types so it serialises cleanly across the Tauri boundary.
/// The `keys`, `values`, and `formats` arrays must be parallel (same length).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataSetArgs {
    /// Branch name; empty string uses the current branch.
    #[serde(default)]
    pub branch: String,
    /// Metadata keys to set (e.g., "description", "owner").
    pub keys: Vec<String>,
    /// Values to set, one per key.
    pub values: Vec<String>,
    /// Value type for each key, one per key. Defaults to String for each
    /// entry if omitted or shorter than `keys`.
    #[serde(default)]
    pub formats: Vec<MetadataFormat>,
}

impl MetadataSetArgs {
    fn into_lore(self) -> LoreBranchMetadataSetArgs {
        // If formats is shorter than keys, pad with String (the common case).
        let formats: Vec<LoreMetadataType> = self
            .keys
            .iter()
            .enumerate()
            .map(|(i, _)| {
                self.formats
                    .get(i)
                    .copied()
                    .unwrap_or(MetadataFormat::String)
                    .into()
            })
            .collect();

        LoreBranchMetadataSetArgs {
            branch: LoreString::from_str(&self.branch),
            keys: lore::interface::LoreArray::from_vec(
                self.keys
                    .into_iter()
                    .map(|k| LoreString::from_str(&k))
                    .collect(),
            ),
            values: lore::interface::LoreArray::from_vec(
                self.values
                    .into_iter()
                    .map(|v| LoreString::from_str(&v))
                    .collect(),
            ),
            formats: lore::interface::LoreArray::from_vec(formats),
        }
    }
}

/// Result returned on successful metadata set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataSetResult {
    /// The branch whose metadata was set.
    pub branch: String,
    /// The keys that were set.
    pub keys: Vec<String>,
    /// The values that were set (parallel with `keys`).
    pub values: Vec<String>,
}

/// Set metadata key-value pairs on a branch.
///
/// Calls the upstream `lore::branch::metadata_set` in-process and returns
/// a typed result indicating which keys were set on which branch.
pub async fn metadata_set(api: &LoreApi, args: MetadataSetArgs) -> Result<MetadataSetResult> {
    let branch = args.branch.clone();
    let keys = args.keys.clone();
    let values = args.values.clone();

    let (callback, rx) = collect_events();

    let status =
        lore::branch::metadata_set(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("metadata_set failed with status {status}"),
        )));
    }

    Ok(MetadataSetResult {
        branch,
        keys,
        values,
    })
}
