//! `LoreApi` facade — holds an open repo working-dir + global-arg defaults.
//!
//! Every operation fn receives `&LoreApi` as its first argument, which provides
//! the `globals()` builder pre-filled with the working directory.

use crate::global::LoreGlobal;
use std::path::PathBuf;

/// The primary handle through which all lore operations are invoked.
#[derive(Clone)]
pub struct LoreApi {
    global: LoreGlobal,
}

impl LoreApi {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            global: LoreGlobal::new(working_dir),
        }
    }

    /// Construct an API from a fully configured [`LoreGlobal`].
    ///
    /// Useful when non-default global flags are required (identity, offline,
    /// in-memory stores, …) — e.g. the integration-test harness drives the
    /// engine headlessly via `LoreGlobal::new(dir).in_memory(true).offline(true)`.
    pub fn from_global(global: LoreGlobal) -> Self {
        Self { global }
    }

    /// Access the mutable global-args builder.
    pub fn global(&self) -> &LoreGlobal {
        &self.global
    }

    /// Return a fresh [`LoreGlobal`] for this API instance.
    pub fn globals(&self) -> LoreGlobal {
        self.global.clone()
    }

    /// Change the working directory (e.g. after opening a different repo).
    pub fn set_working_dir(&mut self, path: PathBuf) {
        self.global.repository_path = path;
    }
}
