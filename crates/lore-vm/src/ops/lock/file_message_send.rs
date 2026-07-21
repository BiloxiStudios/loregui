//! `lock file_message_send` operation — stub.
//!
//! Sends a lock-coordination message to a file's lock holder, relayed via
//! the cloud backend. The holder receives a toast + inbox item and can
//! Release-and-notify or Decline.
//!
//! # Blocking Dependency
//!
//! The upstream `lore` crate (pinned commit `2d86d1dd`, the v0.8.5 tag
//! target) does NOT provide
//! `lore::lock::file_message_send`, nor does `LoreEvent` include a
//! `LockFileMessageSend` variant. The `lore::notification` module has no
//! `publish` method, and `ExtensionEvent` is dropped before becoming a
//! `LoreEvent` (see `docs/lock-messaging-spike.md`, SBAI-4044).
//!
//! The MVP delivers lock messaging locally (process-local inbox + OS tray
//! notification) via the `lock_request_checkin` Tauri command
//! (`src-tauri/src/commands.rs`). Cross-network delivery requires either:
//!
//! 1. **Upstream lore change** — surface `notification::publish` and map
//!    `ExtensionEvent::Other` → `LoreEvent` in the high-level crate; or
//! 2. **Cloud relay side-channel** (SBAI-4072) — `POST /api/v1/lock-messages`
//!
//! Once one of these exists, replace the stub body following the reference
//! pattern in `ops/auth/login_with_token.rs`:
//!   - convert args via `into_lore()`
//!   - call `lore::lock::file_message_send(...)` with event callback
//!   - collect events via `crate::collect::collect_events`
//!   - map `LoreEvent::LockFileMessageSend` → typed result

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
/// `file_status` call.
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
/// # Stub
///
/// This is a stub because the upstream `lore` crate lacks both
/// `lore::lock::file_message_send` and a `LoreEvent::LockFileMessageSend`
/// variant. Lock-coordination messaging is delivered locally (same machine)
/// via the `lock_request_checkin` Tauri command, which pushes to a
/// process-local inbox and fires an OS tray notification.
///
/// Cross-network delivery to another user's client is stubbed behind
/// `TODO(SBAI-4072 relay)` — see `docs/lock-messaging-spike.md` for the
/// full transport analysis.
pub async fn file_message_send(
    _api: &LoreApi,
    _args: FileMessageSendArgs,
) -> Result<FileMessageSendResult> {
    Err(LoreError::CommandFailed(
        "file_message_send: lock-coordination messaging requires either \
         an upstream lore change (notification::publish + ExtensionEvent \
         → LoreEvent mapping) or the cloud relay side-channel \
         (POST /api/v1/lock-messages, SBAI-4072). \
         See docs/lock-messaging-spike.md."
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

    #[tokio::test]
    async fn stub_returns_error_explaining_blocker() {
        let api = LoreApi::new(std::path::PathBuf::from("/nonexistent"));
        let args = FileMessageSendArgs {
            file_path: "test.rs".into(),
            branch: "main".into(),
            to_user_id: "user-1".into(),
            message_type: LockMessageType::RequestUnlock,
            note: String::new(),
        };
        let err = file_message_send(&api, args).await.unwrap_err();
        match err {
            LoreError::CommandFailed(msg) => {
                assert!(
                    msg.contains("SBAI-4072"),
                    "error should reference SBAI-4072: {msg}"
                );
                assert!(
                    msg.contains("lock-messaging-spike.md"),
                    "error should reference spike doc: {msg}"
                );
            }
            other => panic!("expected CommandFailed, got {other:?}"),
        }
    }
}
