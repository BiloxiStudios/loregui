//! `governance::artifact_mark_superseded` — mark a revision's artifact as superseded.
//!
//! Sets a `superseded-by` metadata key on the target revision so downstream
//! consumers (release gates, artifact registries, parity watchers) can reject
//! it in favour of the successor. No upstream `lore` function exists for this;
//! it composes `lore::revision::metadata_set`.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreMetadataType, LoreString};
use lore::revision::{LoreRevisionMetadataSetArgs, metadata_set};
use serde::{Deserialize, Serialize};

/// Metadata key used to record the superseding revision hash.
pub const METADATA_KEY_SUPERSEDED_BY: &str = "superseded-by";
/// Metadata key used to record the reason for supersession.
pub const METADATA_KEY_SUPERSEDED_REASON: &str = "superseded-reason";

/// Arguments for [`artifact_mark_superseded`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArtifactMarkSupersededArgs {
    /// Revision hash to mark as superseded; empty targets the current revision.
    #[serde(default)]
    pub revision: String,
    /// Hash of the revision that supersedes this one.
    pub superseded_by: String,
    /// Human-readable reason for the supersession (e.g. "CVE-2026-1234 fix").
    #[serde(default)]
    pub reason: String,
}

impl ArtifactMarkSupersededArgs {
    fn into_lore(self) -> LoreRevisionMetadataSetArgs {
        let keys = vec![
            LoreString::from_str(METADATA_KEY_SUPERSEDED_BY),
            LoreString::from_str(METADATA_KEY_SUPERSEDED_REASON),
        ];
        let values = vec![
            LoreString::from_str(&self.superseded_by),
            LoreString::from_str(&self.reason),
        ];
        let formats = vec![LoreMetadataType::String, LoreMetadataType::String];

        LoreRevisionMetadataSetArgs {
            keys: lore::interface::LoreArray::from_vec(keys),
            values: lore::interface::LoreArray::from_vec(values),
            formats: lore::interface::LoreArray::from_vec(formats),
        }
    }
}

/// Result returned after marking an artifact as superseded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMarkSupersededResult {
    /// The revision that was marked.
    pub revision: String,
    /// The revision that supersedes it.
    pub superseded_by: String,
    /// The reason recorded.
    pub reason: String,
}

/// Mark a revision's artifact as superseded by setting governance metadata.
///
/// Writes `superseded-by` and `superseded-reason` metadata on the target
/// revision so that downstream governance checks (submission gates, artifact
/// registries) can reject it in favour of the named successor.
pub async fn artifact_mark_superseded(
    api: &LoreApi,
    args: ArtifactMarkSupersededArgs,
) -> Result<ArtifactMarkSupersededResult> {
    let revision_tag = if args.revision.is_empty() {
        "<current>".into()
    } else {
        args.revision.clone()
    };

    let lore_args = args.clone().into_lore();

    let (callback, rx) = collect_events();

    let status = metadata_set(api.globals().build(), lore_args, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("artifact_mark_superseded failed with status {status}"),
        )));
    }

    Ok(ArtifactMarkSupersededResult {
        revision: revision_tag,
        superseded_by: args.superseded_by,
        reason: args.reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_defaults() {
        let args = ArtifactMarkSupersededArgs::default();
        assert!(args.revision.is_empty());
        assert!(args.superseded_by.is_empty());
        assert!(args.reason.is_empty());
    }

    #[test]
    fn args_into_lore_sets_both_keys() {
        let args = ArtifactMarkSupersededArgs {
            revision: "abc123".into(),
            superseded_by: "def456".into(),
            reason: "security fix".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.keys.as_slice().len(), 2);
        assert_eq!(lore_args.values.as_slice().len(), 2);
        assert_eq!(
            lore_args.values.as_slice()[0].as_str(),
            "def456"
        );
        assert_eq!(
            lore_args.values.as_slice()[1].as_str(),
            "security fix"
        );
    }

    #[test]
    fn result_serialises() {
        let result = ArtifactMarkSupersededResult {
            revision: "abc".into(),
            superseded_by: "def".into(),
            reason: "CVE-2026-0001".into(),
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("abc"));
        assert!(json.contains("def"));
        assert!(json.contains("CVE-2026-0001"));
    }

    #[test]
    fn metadata_key_constants() {
        assert_eq!(METADATA_KEY_SUPERSEDED_BY, "superseded-by");
        assert_eq!(METADATA_KEY_SUPERSEDED_REASON, "superseded-reason");
    }
}
