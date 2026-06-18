//! `revision sync` operation — binds `lore::revision::sync`.
//!
//! TODO: implement following the reference op
//! `crates/lore-vm/src/ops/auth/login_with_token.rs` and IMPLEMENTATION-PLAN.md §4:
//!   - build args via `crate::global::LoreGlobal`
//!   - collect events via `crate::collect::collect_events`
//!   - map to a typed result in `crate::model`
//! Acceptance: `cargo check -p lore-vm` + a test. Edit ONLY this file.
