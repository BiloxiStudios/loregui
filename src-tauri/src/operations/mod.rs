//! Notification operations for live branch/lock events.
//!
//! This module provides subscribe/unsubscribe operations that allow the
//! frontend to receive real-time notifications about repository changes
//! (branch switches, new commits, lock acquisitions, etc.) through
//! Tauri's event system.

pub mod subscribe;
pub mod unsubscribe;

/// Types of notification events the frontend can subscribe to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    /// Branch changed (switch, create, delete, merge).
    BranchChange,
    /// New revision committed.
    NewRevision,
    /// Remote sync status changed (ahead/behind).
    SyncStatus,
    /// File lock acquired or released.
    LockChange,
    /// All notification types.
    All,
}

/// Internal subscription registry key type.
pub type SubscriptionId = u64;
