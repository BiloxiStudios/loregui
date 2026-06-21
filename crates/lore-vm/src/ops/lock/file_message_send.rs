//! `lock file_message_send` operation — design spike + stub.
//!
//! Sends a lock-coordination message to a file's lock holder, relayed via
//! the cloud backend. The holder receives a toast + inbox item and can
//! Release-and-notify or Decline.
//!
//! # Design Spike — Transport & Message Model
//!
//! **Transport:** cloud backend exposes `POST /api/v1/lock-messages` relayed
//! through the StudioBrain sync substrate (Valkey pub/sub on `tenant:{id}:sync`
//! extended with `tenant:{id}:messages`). Desktop subscribes when online.
//!
//! **Message model:**
//! - `RequestUnlock` — structured request: "Please release lock on <file>"
//! - `FreeText` — arbitrary note from sender to holder
//!
//! **Boundaries:** identity/relay touches cloud + accounts (PII, auth).
//! LoreGUI only sends/receives, never stores identity/PII.
//!
//! # Blocking Dependency
//!
//! The upstream `lore` crate (EpicGames/lore.git) does NOT provide messaging
//! types or functions. This op cannot bind `lore::lock::*` for messaging
//! because no such module exists. Once lore adds `lore::lock::file_message_send`
//! (or equivalent), this file should be updated to follow the reference pattern
//! in `ops/auth/login_with_token.rs`:
//!   - convert args via `into_lore()`
//!   - call `lore::lock::file_message_send(...)` with event callback
//!   - collect events via `crate::collect::collect_events`
//!   - map `LoreEvent::LockMessageSend` → typed result
//!
//! # Cross-Repo Plan
//!
//! 1. **cloud/accounts** — add `POST /api/v1/lock-messages` endpoint
//!    with Valkey pub/sub relay on `tenant:{id}:messages`
//! 2. **app** (Tauri) — desktop send UI ("Request from <holder>" on locked file)
//! 3. **app** (Tauri) — inbox + toasts + tray badge for unread messages
//! 4. **lore** (upstream) — add `lore::lock::file_message_send` for in-process
//!    binding (optional; cloud REST API is primary transport)
//!
//! See `docs/DESKTOP-CLIENT-FEATURES.md` §4.

use crate::api::LoreApi;
use crate::error::{LoreError, Result};
use serde::{Deserialize, Serialize};

/// Type of lock-coordination message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LockMessageType {
    /// Structured unlock request: "Please release this lock"
    RequestUnlock,
    /// Free-text note from sender to holder.
    FreeText,
}

/// Arguments for [`file_message_send`].
///
/// The sender knows the holder identity from a prior `file_query` or
/// `file_status` call. The cloud relay delivers the message to the holder
/// via Valkey pub/sub on `tenant:{id}:messages`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMessageSendArgs {
    /// File path the lock applies to.
    pub file_path: String,
    /// Branch the lock is on.
    pub branch: String,
    /// Recipient (lock holder) user ID.
    pub to_user_id: String,
    /// Type of message being sent.
    pub message_type: LockMessageType,
    /// Optional note accompanying the message.
    #[serde(default)]
    pub note: String,
}

/// Result returned when a lock message is sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMessageSendResult {
    /// Whether the message was delivered to the holder.
    pub delivered: bool,
    /// Server-assigned message ID for tracking in inbox.
    pub message_id: String,
}

/// Sends a lock-coordination message to the holder of a file lock.
///
/// # Implementation Note
///
/// This operation requires the cloud backend relay endpoint
/// (`POST /api/v1/lock-messages`) which is not yet implemented.
/// The lore crate also lacks messaging types.
///
/// Once the cloud relay is available, this function should POST the
/// message to the endpoint with auth token, then parse the response
/// into `FileMessageSendResult`.
///
/// # Blocking
///
/// Blocked on:
/// - cloud/accounts: relay endpoint implementation
/// - lore crate: `lore::lock::file_message_send` (optional, for in-process binding)
pub async fn file_message_send(
    _api: &LoreApi,
    _args: FileMessageSendArgs,
) -> Result<FileMessageSendResult> {
    Err(LoreError::CommandFailed(
        "file_message_send: cloud relay endpoint not yet implemented. \
         Blocked on cloud/accounts POST /api/v1/lock-messages. \
         See DESKTOP-CLIENT-FEATURES.md §4 and SBAI-4044."
            .into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_deserialise_request_unlock() {
        let args: FileMessageSendArgs = serde_json::from_str(
            r#"{
                "file_path": "src/main.rs",
                "branch": "main",
                "to_user_id": "user-42",
                "message_type": "request_unlock"
            }"#,
        )
        .expect("deserialise");
        assert_eq!(args.file_path, "src/main.rs");
        assert_eq!(args.branch, "main");
        assert_eq!(args.to_user_id, "user-42");
        assert!(matches!(args.message_type, LockMessageType::RequestUnlock));
        assert!(args.note.is_empty());
    }

    #[test]
    fn args_deserialise_free_text_with_note() {
        let args: FileMessageSendArgs = serde_json::from_str(
            r#"{
                "file_path": "lib/core.verse",
                "branch": "feature-x",
                "to_user_id": "user-99",
                "message_type": "free_text",
                "note": "Can you release this when done? I need it for the merge."
            }"#,
        )
        .expect("deserialise");
        assert_eq!(args.file_path, "lib/core.verse");
        assert!(matches!(args.message_type, LockMessageType::FreeText));
        assert!(args.note.contains("release"));
    }

    #[test]
    fn args_serialise_roundtrip() {
        let args = FileMessageSendArgs {
            file_path: "test.rs".into(),
            branch: "dev".into(),
            to_user_id: "u-1".into(),
            message_type: LockMessageType::RequestUnlock,
            note: "please release".into(),
        };
        let json = serde_json::to_string(&args).expect("serialise");
        let parsed: FileMessageSendArgs = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(parsed.file_path, args.file_path);
        assert_eq!(parsed.to_user_id, args.to_user_id);
        assert!(matches!(
            parsed.message_type,
            LockMessageType::RequestUnlock
        ));
    }

    #[test]
    fn result_serialises() {
        let result = FileMessageSendResult {
            delivered: true,
            message_id: "msg-abc-123".into(),
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("msg-abc-123"));
        assert!(json.contains("true"));
    }

    #[test]
    fn message_type_variants_serialise() {
        assert_eq!(
            serde_json::to_string(&LockMessageType::RequestUnlock).unwrap(),
            "\"request_unlock\""
        );
        assert_eq!(
            serde_json::to_string(&LockMessageType::FreeText).unwrap(),
            "\"free_text\""
        );
    }
}
