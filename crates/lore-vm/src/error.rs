use serde::Serialize;

/// Errors surfaced by a [`crate::backend::LoreBackend`].
///
/// Serializable so Tauri commands can return them straight to the frontend.
#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum LoreError {
    /// The `lore` binary could not be found or failed to launch.
    #[error("lore CLI not found or failed to launch: {0}")]
    CliUnavailable(String),

    /// A `lore` invocation exited non-zero (or upstream API returned failure).
    #[error("lore command failed: {0}")]
    CommandFailed(String),

    /// Output from `lore` could not be parsed into the expected shape.
    #[error("could not parse lore output: {0}")]
    Parse(String),

    /// No working tree / repository at the configured path.
    #[error("no Lore repository at the configured path: {0}")]
    NoRepository(String),

    /// Anything from the in-process lore-client adapter.
    #[error("lore client error: {0}")]
    Client(String),

    /// Authentication-specific error (token expired, invalid, auth URL unreachable).
    #[error("auth error: {0}")]
    Auth(String),
}

pub type Result<T> = std::result::Result<T, LoreError>;
