//! `repository instance_prune` operation — binds `lore::repository::instance_prune`.
//!
//! Removes stale repository instances (working copies whose filesystem paths no
//! longer exist) from the shared store.  Each pruned instance is reported via a
//! `LoreEvent::RepositoryInstance` event; the binding collects these and returns
//! the count plus details of every pruned instance.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::repository::LoreRepositoryInstancePruneArgs;
use serde::{Deserialize, Serialize};

/// A single pruned instance entry returned to the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrunedInstance {
    /// Hex-encoded instance ID.
    pub instance_id: String,
    /// Filesystem path that was registered for this instance.
    pub path: String,
    /// Branch name the instance had checked out (may be empty).
    pub branch_name: String,
}

/// Result of a successful `instance_prune` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstancePruneResult {
    /// Number of stale instances that were removed.
    pub pruned_count: u32,
    /// Details of each pruned instance.
    pub pruned: Vec<PrunedInstance>,
}

/// Prune stale repository instances from the shared store.
///
/// Calls upstream `lore::repository::instance_prune` in-process, collects
/// the `RepositoryInstance` events emitted for each pruned entry, and returns
/// a typed result with the count and details.
pub async fn instance_prune(api: &LoreApi) -> Result<InstancePruneResult> {
    let args = LoreRepositoryInstancePruneArgs {};
    let (callback, rx) = collect_events();

    let status = lore::repository::instance_prune(api.globals().build(), args, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("instance_prune failed with status {status}"),
        )));
    }

    let mut pruned = Vec::new();
    for event in &stream.events {
        if let LoreEvent::RepositoryInstance(data) = event {
            pruned.push(PrunedInstance {
                instance_id: format!("{}", data.instance_id),
                path: data.path.as_str().to_string(),
                branch_name: data.branch_name.as_str().to_string(),
            });
        }
    }

    Ok(InstancePruneResult {
        pruned_count: pruned.len() as u32,
        pruned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = InstancePruneResult {
            pruned_count: 1,
            pruned: vec![PrunedInstance {
                instance_id: "abc123".into(),
                path: "/tmp/gone".into(),
                branch_name: "main".into(),
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"pruned_count\":1"));
        assert!(json.contains("\"instance_id\":\"abc123\""));
    }

    #[test]
    fn empty_prune_result() {
        let result = InstancePruneResult {
            pruned_count: 0,
            pruned: vec![],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"pruned_count\":0"));
        assert!(json.contains("\"pruned\":[]"));
    }
}
