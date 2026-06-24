//! Shared helpers for the `storage`/`shared_store` behavioral integration
//! suites. Placed under `tests/storage_support/` (a subdirectory module, not a
//! top-level `tests/*.rs`) so cargo does NOT treat it as its own test binary —
//! it is `mod storage_support;`-included by each suite that needs it.
//!
//! Only compiled when the `integration-tests` feature is on; each including
//! suite is itself `#![cfg(feature = "integration-tests")]`, and these helpers
//! are gated too so a normal `cargo test -p lore-vm` never builds them.
#![cfg(feature = "integration-tests")]
#![allow(dead_code)]

use std::path::Path;

use lore_vm::api::LoreApi;
use lore_vm::global::LoreGlobal;
use lore_vm::ops;

/// A canonical onboarding-style partition (mirrors the GUI connectivity check).
pub const PARTITION: &str = "00000000000000000000000000000001";
/// A second distinct partition for multi-partition addressing assertions.
pub const PARTITION_TWO: &str = "00000000000000000000000000000002";

/// Build a `LoreApi` for headless **on-disk** operation as `identity`:
/// a real `.urc` store on disk, no server (`offline = 1`), no in-memory store.
pub fn on_disk_api(dir: &Path, identity: &str) -> LoreApi {
    let global = LoreGlobal::new(dir.to_path_buf())
        .in_memory(false)
        .offline(true)
        .identity(identity);
    LoreApi::from_global(global)
}

/// Write `contents` to `path`, creating parent directories as needed.
pub fn write_file(path: &Path, contents: &[u8]) {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dirs");
    }
    let mut f = std::fs::File::options()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .expect("create file");
    f.write_all(contents).expect("write file");
}

/// A fully created, on-disk lore repository backed by a shared store that lives
/// OUTSIDE the working tree — the closest stand-in for a self-hosted layout.
/// Holds the tempdirs so they outlive the test body.
pub struct DiskRepo {
    pub api: LoreApi,
    /// The repository working tree.
    pub work: tempfile::TempDir,
    /// The directory containing the shared store (kept disjoint from `work`).
    pub store: tempfile::TempDir,
    /// Absolute path to the repo working tree (== `work.path()`).
    pub repo_path: std::path::PathBuf,
    /// Absolute path to the shared store.
    pub store_path: std::path::PathBuf,
    /// The created repository id.
    pub repo_id: String,
}

/// Create a fresh on-disk repo named `<name_prefix>-<pid>-<unique>` backed by a
/// shared store outside the working tree, authored by `identity`.
pub async fn create_disk_repo(name_prefix: &str, identity: &str) -> DiskRepo {
    let work = tempfile::tempdir().expect("create work tempdir");
    let store = tempfile::tempdir().expect("create store tempdir");
    let repo_path = work.path().to_path_buf();
    let store_path = store.path().join("shared-store");

    let api = on_disk_api(&repo_path, identity);

    ops::shared_store::create::create(
        &api,
        ops::shared_store::create::SharedStoreCreateArgs {
            remote_url: String::new(),
            path: Some(store_path.to_string_lossy().into_owned()),
            make_default: false,
        },
    )
    .await
    .expect("shared_store::create should succeed");

    let unique = next_unique();
    let repo_url = format!(
        "lore://localhost/{name_prefix}-{}-{unique}",
        std::process::id()
    );
    let created = ops::repository::create::create(
        &api,
        ops::repository::create::CreateArgs {
            repository_url: repo_url,
            description: format!("lore-vm storage-coverage repo ({name_prefix})"),
            id: String::new(),
            use_shared_store: true,
            shared_store_path: store_path.to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("repository::create should succeed");
    assert!(
        !created.id.is_empty(),
        "repository::create returned an empty id: {created:?}"
    );

    DiskRepo {
        api,
        work,
        store,
        repo_path,
        store_path,
        repo_id: created.id,
    }
}

/// Join an existing repo (same shared store + same id) on a fresh working tree
/// authored by `identity`. Used to exercise cross-repo shared-store sync.
pub async fn join_disk_repo(
    repo_url: &str,
    repo_id: &str,
    store_path: &Path,
    identity: &str,
) -> (LoreApi, tempfile::TempDir, std::path::PathBuf) {
    let work = tempfile::tempdir().expect("create joiner work tempdir");
    let repo_path = work.path().to_path_buf();
    let api = on_disk_api(&repo_path, identity);

    let created = ops::repository::create::create(
        &api,
        ops::repository::create::CreateArgs {
            repository_url: repo_url.to_string(),
            description: "lore-vm storage-coverage joiner".into(),
            id: repo_id.to_string(),
            use_shared_store: true,
            shared_store_path: store_path.to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("repository::create (joiner, same id) should succeed");
    assert_eq!(
        created.id, repo_id,
        "joiner must join the same repository id"
    );

    (api, work, repo_path)
}

/// Stage a single absolute path and assert the op succeeded.
pub async fn stage_path(api: &LoreApi, path: &Path) -> ops::file::stage::FileStageResult {
    ops::file::stage::stage(
        api,
        ops::file::stage::FileStageArgs {
            paths: vec![path.to_string_lossy().into_owned()],
            case_change: ops::file::stage::CaseChange::Error,
            scan: true,
        },
    )
    .await
    .unwrap_or_else(|e| panic!("file::stage should succeed for {}: {e}", path.display()))
}

/// Commit the staged state, asserting a non-empty revision hash, and return it.
pub async fn commit(api: &LoreApi, message: &str) -> String {
    let res = ops::revision::commit::commit(
        api,
        ops::revision::commit::CommitArgs {
            message: message.into(),
        },
    )
    .await
    .unwrap_or_else(|e| panic!("revision::commit({message:?}) should succeed: {e}"));
    assert!(
        !res.revision.is_empty(),
        "commit({message:?}) returned an empty revision hash: {res:?}"
    );
    res.revision
}

/// Full revision history for the current branch, newest-first.
pub async fn history(api: &LoreApi) -> ops::revision::history::RevisionHistoryResult {
    ops::revision::history::history(api, ops::revision::history::RevisionHistoryArgs::default())
        .await
        .expect("revision::history should succeed")
}

/// Sync the current repo against its shared store (offline) and return the tip
/// revisions observed.
pub async fn sync(api: &LoreApi) -> ops::revision::sync::RevisionSyncResult {
    ops::revision::sync::sync(api, ops::revision::sync::RevisionSyncArgs::default())
        .await
        .expect("revision::sync should succeed against the shared store")
}

/// Process-local monotonic counter so repos created within one test binary get
/// distinct urls/ids even at the same pid.
fn next_unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}
