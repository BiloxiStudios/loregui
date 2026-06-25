//! `host_store`-domain operations — one sub-module per op.
//!
//! Filesystem-native ops for the "Host a server" local-FS wizard path.
//! These operate directly on the filesystem (no lore crate binding), providing
//! a safe prepare (create store dir) and probe (writability round-trip) for
//! local host-store directories. Reference: ops/auth/login_with_token.rs.

pub mod prepare;
pub mod probe;
