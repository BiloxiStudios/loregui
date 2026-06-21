//! `lock`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::lock::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod file_acquire;
pub mod file_acquire_as_owner;
pub mod file_message_send;
pub mod file_query;
pub mod file_release;
pub mod file_status;
