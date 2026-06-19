//! `repository create_with_metadata` operation — binds `lore::repository::create_with_metadata`.
//!
//! Creates a new repository with explicitly provided creator and timestamp metadata.
//! This is typically used for mirroring or importing where the original creation
//! context must be preserved.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::repository::{LoreRepositoryCreateArgs, LoreRepositoryCreateMetadata};
use serde::{Deserialize, Serialize};

/// Arguments for [`create_with_metadata`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWithMetadataArgs {
    /// URL to the repository.
    pub repository_url: String,
    /// Optional repository description.
    #[serde(default)]
    pub description: String,
    /// Optional repository ID (UUID); empty string generates a new one.
    #[serde(default)]
    pub id: String,
    /// Use the shared store instead of a local immutable store.
    #[serde(default)]
    pub use_shared_store: bool,
    /// Optional path for the shared store.
    #[serde(default)]
    pub shared_store_path: String,
    /// Identity to attribute repository creation to.
    pub creator: String,
    /// Creation timestamp (milliseconds since the Unix epoch).
    pub created: u64,
}

impl CreateWithMetadataArgs {
    fn into_lore(self) -> (LoreRepositoryCreateArgs, LoreRepositoryCreateMetadata) {
        let args = LoreRepositoryCreateArgs {
            repository_url: LoreString::from_str(&self.repository_url),
            description: LoreString::from_str(&self.description),
            id: LoreString::from_str(&self.id),
            use_shared_store: if self.use_shared_store { 1 } else { 0 },
            shared_store_path: LoreString::from_str(&self.shared_store_path),
        };
        let metadata = LoreRepositoryCreateMetadata {
            creator: LoreString::from_str(&self.creator),
            created: self.created,
        };
        (args, metadata)
    }
}

/// Result of a successful `create_with_metadata` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWithMetadataResult {
    /// Identifier of the created repository.
    pub id: String,
    /// Name of the created repository.
    pub name: String,
    /// Local path of the created repository.
    pub path: String,
}

/// Create a new repository with explicitly provided creator and timestamp metadata.
///
/// Calls the upstream `lore::repository::create_with_metadata` in-process and
/// collects the `RepositoryCreate` event to return a typed result.
pub async fn create_with_metadata(
    api: &LoreApi,
    args: CreateWithMetadataArgs,
) -> Result<CreateWithMetadataResult> {
    let (args, metadata) = args.into_lore();
    let (callback, rx) = collect_events();

    let status =
        lore::repository::create_with_metadata(api.globals().build(), args, metadata, callback)
            .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("create_with_metadata failed with status {status}"),
        )));
    }

    for event in &stream.events {
        if let LoreEvent::RepositoryCreate(data) = event {
            return Ok(CreateWithMetadataResult {
                id: format!("{}", data.id),
                name: data.name.as_str().to_string(),
                path: data.path.as_str().to_string(),
            });
        }
    }

    Err(LoreError::Parse(
        "repository created successfully but no RepositoryCreate event emitted".into(),
    ))
}
