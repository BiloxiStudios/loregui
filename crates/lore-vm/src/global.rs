//! Builder for [`lore::interface::LoreGlobalArgs`].
//!
//! Holds repository path, identity, offline/force flags, and parallelism limits.
//! Every operation fn receives a `LoreGlobalArgs` built from this helper.
//!
//! ## Identity mutability (SBAI-4933)
//!
//! The `identity` field uses interior mutability (`std::sync::RwLock<String>`)
//! so that the verified subject from a server-authenticated JWT (returned by
//! `auth::login_with_token`) can be propagated into the global args *after*
//! the `LoreGlobal` / `LoreApi` has been constructed. This ensures that commit
//! attribution always uses the server-verified user identity, not an untrusted
//! client-decoded payload.
//!
//! `RwLock` (not `RefCell`) is required because the async ops capture `&LoreApi`
//! across `.await` points, making the future `Send`-bound, which requires
//! `LoreApi: Sync`.

use lore::interface::LoreGlobalArgs;
use lore::interface::LoreString;
use std::path::PathBuf;
use std::sync::RwLock;

/// Builder for global args shared by all Lore operations.
#[derive(Debug)]
pub struct LoreGlobal {
    pub repository_path: PathBuf,
    /// The identity used for commit attribution. Uses interior mutability so
    /// `auth::login_with_token` can set the verified user-id after the API
    /// handle has been constructed (SBAI-4933).
    identity: RwLock<String>,
    pub offline: bool,
    pub force: bool,
    pub max_connections: u32,
    /// Run with in-process, in-memory immutable/mutable stores (no on-disk
    /// `.urc` store and no server). Used by the integration-test harness to
    /// drive the real lore engine headlessly. Mirrors
    /// [`LoreGlobalArgs::in_memory`].
    pub in_memory: bool,
}

// Manual Clone — `RwLock` does not implement `Clone`.
impl Clone for LoreGlobal {
    fn clone(&self) -> Self {
        Self {
            repository_path: self.repository_path.clone(),
            identity: RwLock::new(self.identity.read().unwrap().clone()),
            offline: self.offline,
            force: self.force,
            max_connections: self.max_connections,
            in_memory: self.in_memory,
        }
    }
}

impl LoreGlobal {
    pub fn new(repository_path: PathBuf) -> Self {
        Self {
            repository_path,
            identity: RwLock::new(String::new()),
            offline: false,
            force: false,
            max_connections: 8,
            in_memory: false,
        }
    }

    pub fn identity(self, id: impl Into<String>) -> Self {
        *self.identity.write().unwrap() = id.into();
        self
    }

    /// Set the identity after construction. Used by `auth::login_with_token`
    /// to propagate the server-verified user-id into the global args so that
    /// all subsequent commit attribution uses the authenticated subject
    /// (SBAI-4933).
    pub fn set_identity(&self, id: impl Into<String>) {
        *self.identity.write().unwrap() = id.into();
    }

    /// Read the current identity value.
    pub fn get_identity(&self) -> String {
        self.identity.read().unwrap().clone()
    }

    pub fn offline(mut self, v: bool) -> Self {
        self.offline = v;
        self
    }

    pub fn force(mut self, v: bool) -> Self {
        self.force = v;
        self
    }

    pub fn max_connections(mut self, v: u32) -> Self {
        self.max_connections = v;
        self
    }

    pub fn in_memory(mut self, v: bool) -> Self {
        self.in_memory = v;
        self
    }

    /// Build the [`LoreGlobalArgs`] expected by the lore crate's async fns.
    pub fn build(&self) -> LoreGlobalArgs {
        LoreGlobalArgs {
            repository_path: LoreString::from_path(&self.repository_path),
            correlation_id: LoreString::default(),
            identity: LoreString::from_str(&self.identity.read().unwrap()),
            force: u8::from(self.force),
            offline: u8::from(self.offline),
            local: 0,
            remote: 0,
            dry_run: 0,
            no_atime: 0,
            max_connections: self.max_connections,
            search_limit: 100,
            search_nearest: 0,
            gc: 0,
            in_memory: u8::from(self.in_memory),
            // Remaining fields (file_count_limit, file_size_limit, compress_task_limit,
            // store_keep_alive*, sync_data, cache) take their upstream defaults.
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_builder_sets_value() {
        let g = LoreGlobal::new(PathBuf::from("/tmp")).identity("alice");
        assert_eq!(g.get_identity(), "alice");
    }

    #[test]
    fn set_identity_after_construction() {
        let g = LoreGlobal::new(PathBuf::from("/tmp"));
        assert_eq!(g.get_identity(), "");
        g.set_identity("bob");
        assert_eq!(g.get_identity(), "bob");
    }

    #[test]
    fn clone_copies_identity() {
        let g = LoreGlobal::new(PathBuf::from("/tmp")).identity("carol");
        let g2 = g.clone();
        assert_eq!(g2.get_identity(), "carol");
        // Independent mutation after clone.
        g.set_identity("dave");
        assert_eq!(g.get_identity(), "dave");
        assert_eq!(g2.get_identity(), "carol");
    }

    #[test]
    fn build_uses_current_identity() {
        let g = LoreGlobal::new(PathBuf::from("/tmp")).identity("eve");
        let args = g.build();
        assert_eq!(args.identity.as_str(), "eve");
        // Mutate and rebuild.
        g.set_identity("frank");
        let args2 = g.build();
        assert_eq!(args2.identity.as_str(), "frank");
    }
}
