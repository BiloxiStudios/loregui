//! Builder for [`lore::interface::LoreGlobalArgs`].
//!
//! Holds repository path, identity, offline/force flags, and parallelism limits.
//! Every operation fn receives a `LoreGlobalArgs` built from this helper.

use lore::interface::LoreGlobalArgs;
use lore::interface::LoreString;
use std::path::PathBuf;

/// Builder for global args shared by all Lore operations.
#[derive(Debug, Clone)]
pub struct LoreGlobal {
    pub repository_path: PathBuf,
    pub identity: String,
    pub offline: bool,
    pub force: bool,
    pub max_connections: u32,
    /// Run with in-process, in-memory immutable/mutable stores (no on-disk
    /// `.urc` store and no server). Used by the integration-test harness to
    /// drive the real lore engine headlessly. Mirrors
    /// [`LoreGlobalArgs::in_memory`].
    pub in_memory: bool,
}

impl LoreGlobal {
    pub fn new(repository_path: PathBuf) -> Self {
        Self {
            repository_path,
            identity: String::new(),
            offline: false,
            force: false,
            max_connections: 8,
            in_memory: false,
        }
    }

    pub fn identity(mut self, id: impl Into<String>) -> Self {
        self.identity = id.into();
        self
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
            identity: LoreString::from_str(&self.identity),
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
