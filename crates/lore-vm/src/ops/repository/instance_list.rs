//! `repository instance_list` operation — binds `lore::repository::instance_list`.
//!
//! Lists all registered instances (working copies) of a repository from the
//! shared store. Each instance is reported via a `LoreEvent::RepositoryInstance`
//! event; the binding collects these and returns a typed list with instance
//! metadata including staleness, branch, and revision info.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::repository::LoreRepositoryInstanceListArgs;
use serde::{Deserialize, Serialize};

/// A single registered instance entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceEntry {
    /// Hex-encoded instance ID.
    pub instance_id: String,
    /// Filesystem path registered for this instance.
    pub path: String,
    /// Branch name the instance has checked out (may be empty).
    pub branch_name: String,
    /// Branch identifier (hex).
    pub branch: String,
    /// Current revision hash (hex, empty when unset).
    pub revision: String,
    /// True when the instance's filesystem path no longer exists.
    pub stale: bool,
}

/// Result of a successful `instance_list` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceListResult {
    /// Total number of registered instances.
    pub instance_count: u32,
    /// Details of each instance.
    pub instances: Vec<InstanceEntry>,
}

/// List all registered instances of the repository.
///
/// Calls upstream `lore::repository::instance_list` in-process, collects the
/// `RepositoryInstance` events emitted for each entry, and returns a typed
/// result with the count and details.
pub async fn instance_list(api: &LoreApi) -> Result<InstanceListResult> {
    let args = LoreRepositoryInstanceListArgs {};
    let (callback, rx) = collect_events();

    let status = lore::repository::instance_list(api.globals().build(), args, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("instance_list failed with status {status}"),
        )));
    }

    let mut instances = Vec::new();
    for event in &stream.events {
        if let LoreEvent::RepositoryInstance(data) = event {
            instances.push(InstanceEntry {
                instance_id: format!("{}", data.instance_id),
                path: data.path.as_str().to_string(),
                branch_name: data.branch_name.as_str().to_string(),
                branch: format!("{}", data.branch),
                revision: format!("{}", data.revision),
                stale: data.stale != 0,
            });
        }
    }

    Ok(InstanceListResult {
        instance_count: instances.len() as u32,
        instances,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = InstanceListResult {
            instance_count: 1,
            instances: vec![InstanceEntry {
                instance_id: "abc123".into(),
                path: "/tmp/repo".into(),
                branch_name: "main".into(),
                branch: "def456".into(),
                revision: "789aaa".into(),
                stale: false,
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"instance_count\":1"));
        assert!(json.contains("\"instance_id\":\"abc123\""));
        assert!(json.contains("\"stale\":false"));
    }

    #[test]
    fn empty_result() {
        let result = InstanceListResult {
            instance_count: 0,
            instances: vec![],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"instance_count\":0"));
        assert!(json.contains("\"instances\":[]"));
    }

    #[test]
    fn stale_instance_serialises() {
        let result = InstanceListResult {
            instance_count: 1,
            instances: vec![InstanceEntry {
                instance_id: "stale1".into(),
                path: "/gone/path".into(),
                branch_name: "feature".into(),
                branch: "br1".into(),
                revision: "rev1".into(),
                stale: true,
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"stale\":true"));
        assert!(json.contains("\"branch_name\":\"feature\""));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"instance_count":1,"instances":[{"instance_id":"id1","path":"/p","branch_name":"main","branch":"b1","revision":"r1","stale":false}]}"#;
        let result: InstanceListResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.instance_count, 1);
        assert_eq!(result.instances[0].instance_id, "id1");
        assert!(!result.instances[0].stale);
    }
}
