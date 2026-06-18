//! View-model types. Deliberately UI- and transport-agnostic so the same shapes
//! serialize to the Tauri frontend, to StudioBrain, or to anything else.

use serde::{Deserialize, Serialize};

/// How a file differs from the committed revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Untracked,
}

/// A single changed path in the working tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub kind: ChangeKind,
    /// True once `stage` has included this path in the next revision.
    pub staged: bool,
}

/// Snapshot of the working tree — what the GUI's main panel renders.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoStatus {
    pub repo_id: String,
    pub branch: String,
    /// Short form of the current revision hash (BLAKE3).
    pub revision: String,
    pub changes: Vec<FileChange>,
    /// Revisions committed locally but not yet pushed.
    pub ahead: u32,
    /// Revisions on the remote not yet synced locally.
    pub behind: u32,
}

impl RepoStatus {
    pub fn staged(&self) -> impl Iterator<Item = &FileChange> {
        self.changes.iter().filter(|c| c.staged)
    }
    pub fn unstaged(&self) -> impl Iterator<Item = &FileChange> {
        self.changes.iter().filter(|c| !c.staged)
    }
}

/// A branch pointer (Lore's mutable KV store).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub id: String,
    pub latest_revision: String,
    pub is_current: bool,
}

/// One entry in the immutable revision chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    /// BLAKE3 hash signature of the revision.
    pub hash: String,
    pub message: String,
    pub author: String,
    /// RFC3339 timestamp.
    pub timestamp: String,
    pub parent: Option<String>,
}

// ===== Repository domain types =====

/// Detailed information about a Lore repository.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoInfo {
    pub repo_id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub current_branch: String,
    pub current_revision: String,
    pub shared_store_url: Option<String>,
}

/// Key-value metadata attached to a repository.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoMetadata {
    pub entries: Vec<MetadataEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataEntry {
    pub key: String,
    pub value: String,
}

/// A repository listing entry (from `repository list`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoListing {
    pub repo_id: String,
    pub name: String,
    pub path: String,
    pub is_current: bool,
}

/// Result of a verify_state operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyStateResult {
    pub is_valid: bool,
    pub issues: Vec<String>,
}

/// Result of a verify_fragment operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyFragmentResult {
    pub fragment_hash: String,
    pub is_valid: bool,
    pub expected_size: u64,
    pub actual_size: Option<u64>,
}

/// Result of store_immutable_query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImmutableQueryResult {
    pub matches: Vec<ImmutableMatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImmutableMatch {
    pub hash: String,
    pub size: u64,
    pub path: String,
}

/// Repository configuration value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigValue {
    pub key: String,
    pub value: String,
    pub source: String,
}

/// Instance listing for a repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceList {
    pub instances: Vec<InstanceInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub instance_id: String,
    pub name: String,
    pub path: String,
    pub is_active: bool,
}

/// Result of an instance_prune operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstancePruneResult {
    pub pruned_count: u32,
    pub freed_bytes: u64,
}

/// Dump of repository data (serialized state).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoDump {
    pub format: String,
    pub data: String,
}

/// Repository creation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoCreateResult {
    pub repo_id: String,
    pub path: String,
}

// ===== Link domain types (multi-repo composition) =====

/// A link entry — a composition pointer to another Lore repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkEntry {
    /// Identifier of the linked repository.
    pub link: String,
    /// Path of the link within the parent repository.
    pub link_path: String,
    /// Source path within the linked repository.
    pub source_path: String,
    /// Branch name the link is pinned to.
    pub branch_name: String,
    /// Hash of the revision the link is pinned to.
    pub revision: String,
}

/// Result of a link_add operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkAddResult {
    pub link_path: String,
}

/// Result of a link_remove operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRemoveResult {
    pub link_path: String,
}

/// Result of a link_update operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkUpdateResult {
    pub link_path: String,
}

/// Result of listing links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkListResult {
    pub links: Vec<LinkEntry>,
}

/// Result of listing staged link changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkListStagedResult {
    pub links: Vec<LinkEntry>,
}
