//! `repository create` operation — binds `lore::repository::create`.
//!
//! Creates a new repository at the configured working directory. Emits
//! `LoreEvent::RepositoryCreate` carrying the new repository id, name, and path.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::repository::LoreRepositoryCreateArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`create`].
///
/// Mirrors `LoreRepositoryCreateArgs` from the upstream `lore` crate but uses
/// plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateArgs {
    /// URL to the repository (e.g. `lore://localhost/<name>`).
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
}

impl CreateArgs {
    fn into_lore(self) -> LoreRepositoryCreateArgs {
        LoreRepositoryCreateArgs {
            repository_url: LoreString::from_str(&self.repository_url),
            description: LoreString::from_str(&self.description),
            id: LoreString::from_str(&self.id),
            use_shared_store: u8::from(self.use_shared_store),
            shared_store_path: LoreString::from_str(&self.shared_store_path),
        }
    }
}

/// Result of a successful `create` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResult {
    /// Identifier of the created repository.
    pub id: String,
    /// Name of the created repository.
    pub name: String,
    /// Local path of the created repository.
    pub path: String,
}

/// Create a new repository.
///
/// Calls the upstream `lore::repository::create` in-process and collects the
/// `RepositoryCreate` event to return a typed result.
pub async fn create(api: &LoreApi, args: CreateArgs) -> Result<CreateResult> {
    let (callback, rx) = collect_events();

    let status = lore::repository::create(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("repository create failed with status {status}"),
        )));
    }

    for event in &stream.events {
        if let LoreEvent::RepositoryCreate(data) = event {
            return Ok(CreateResult {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_args_serializes() {
        let args = CreateArgs {
            repository_url: "lore://localhost/demo".into(),
            description: "demo repo".into(),
            id: String::new(),
            use_shared_store: false,
            shared_store_path: String::new(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("lore://localhost/demo"));
    }

    #[test]
    fn create_args_deserializes_with_defaults() {
        let json = r#"{"repository_url":"lore://localhost/x"}"#;
        let args: CreateArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.repository_url, "lore://localhost/x");
        assert_eq!(args.description, "");
        assert!(!args.use_shared_store);
    }

    #[test]
    fn create_args_into_lore_conversion() {
        let args = CreateArgs {
            repository_url: "lore://localhost/y".into(),
            description: "d".into(),
            id: "id1".into(),
            use_shared_store: true,
            shared_store_path: "/tmp/store".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.repository_url.as_str(), "lore://localhost/y");
        assert_eq!(lore_args.use_shared_store, 1);
        assert_eq!(lore_args.shared_store_path.as_str(), "/tmp/store");
    }

    #[test]
    fn create_result_serializes() {
        let result = CreateResult {
            id: "repo-1".into(),
            name: "demo".into(),
            path: "/tmp/demo".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("repo-1"));
        assert!(json.contains("demo"));
    }
}
