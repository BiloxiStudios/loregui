//! `relay`-domain operations — one sub-module per op.
//!
//! Relay ops prepare the bore-tunnel configuration for premium cross-network
//! server hosting (SBAI-4072). They are pure computation ops: no CLI shelling,
//! no process spawning. The src-tauri integration manager invokes them to obtain
//! tunnel parameters (auth token, relay URL, remote port), then handles the
//! actual `bore` client lifecycle (spawn / monitor / teardown).
//!
//! Reference: ops/relay/relay_open.rs.

pub mod relay_open;
