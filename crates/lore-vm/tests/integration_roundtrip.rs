//! Runtime integration-test harness for `lore-vm`'s op bindings.
//!
//! Unlike the per-op unit tests (which only check arg/result *serialisation*),
//! this harness drives the REAL in-process `lore` engine through the public
//! `lore-vm` API and asserts on the typed results it returns. It is the only
//! place that proves the op bindings are wired correctly at runtime — that the
//! right upstream fn is called, the right events are collected, and the right
//! fields are mapped.
//!
//! It runs the lore engine in **in-memory mode** (`in_memory = 1`,
//! `offline = 1`): the immutable/mutable stores live in a process-wide cache
//! instead of on disk, and crucially persist across sequential library calls,
//! so a create → stage → commit → status → branch → history round trip works
//! entirely headlessly with no server and no `.urc` store on disk. This mirrors
//! upstream lore's own `lore/tests/in_memory.rs`.
//!
//! Gated behind the `integration-tests` cargo feature so the normal fast
//! `cargo test -p lore-vm` never builds or runs it. Run with:
//!
//! ```sh
//! cargo test -p lore-vm --features integration-tests
//! ```
#![cfg(feature = "integration-tests")]

use std::io::Write;
use std::path::Path;

use lore_vm::api::LoreApi;
use lore_vm::global::LoreGlobal;
use lore_vm::ops;

/// Build a `LoreApi` pointed at `dir`, configured for headless in-memory
/// operation (no server, no on-disk store).
fn in_memory_api(dir: &Path) -> LoreApi {
    let global = LoreGlobal::new(dir.to_path_buf())
        .in_memory(true)
        .offline(true)
        .identity("integration-test");
    LoreApi::from_global(global)
}

/// Write `contents` to `path`, creating parent directories as needed.
fn write_file(path: &Path, contents: &[u8]) {
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

/// Full create → stage → commit → status → branch → history round trip,
/// asserting on the REAL typed results returned by each op binding.
#[tokio::test]
async fn full_roundtrip_against_real_lore() {
    let tempdir = tempfile::tempdir().expect("create temp dir");
    let repo_path = tempdir.path().to_path_buf();
    let api = in_memory_api(&repo_path);

    // ---- 1. create the repository -------------------------------------------
    let name = format!("itest-{}", std::process::id());
    let create = ops::repository::create::create(
        &api,
        ops::repository::create::CreateArgs {
            repository_url: format!("lore://localhost/{name}"),
            description: "lore-vm integration test repo".into(),
            id: String::new(),
            use_shared_store: false,
            shared_store_path: String::new(),
        },
    )
    .await
    .expect("repository::create should succeed against the real engine");
    assert!(
        !create.id.is_empty(),
        "create returned an empty repository id: {create:?}"
    );

    // ---- 2. write a file in the working tree and stage it -------------------
    let file_path = repo_path.join("hello.txt");
    write_file(&file_path, b"hello lore-vm");

    let stage = ops::file::stage::stage(
        &api,
        ops::file::stage::FileStageArgs {
            // Stage absolute filesystem paths, matching upstream's in-memory test.
            paths: vec![file_path.to_string_lossy().into_owned()],
            case_change: ops::file::stage::CaseChange::Error,
            scan: true,
        },
    )
    .await
    .expect("file::stage should succeed");
    assert!(
        stage.files.iter().any(|f| f.path.ends_with("hello.txt")),
        "stage did not report hello.txt: {stage:?}"
    );

    // ---- 3. commit the staged file ------------------------------------------
    let commit = ops::revision::commit::commit(
        &api,
        ops::revision::commit::CommitArgs {
            message: "initial commit".into(),
        },
    )
    .await
    .expect("revision::commit should succeed");
    assert!(
        !commit.revision.is_empty(),
        "commit returned an empty revision hash: {commit:?}"
    );
    let committed_revision = commit.revision.clone();

    // ---- 4. status should report the committed file -------------------------
    let status = ops::repository::status::status(
        &api,
        ops::repository::status::RepositoryStatusArgs {
            staged: true,
            ..Default::default()
        },
    )
    .await
    .expect("repository::status should succeed");
    // After commit the file is part of the current revision; assert the status
    // call returns coherent revision context for the branch it was committed on.
    let rev = status
        .revision
        .as_ref()
        .expect("status should report revision context");
    assert!(
        !rev.branch_name.is_empty(),
        "status reported an empty branch name: {status:?}"
    );

    // ---- 5. create a branch and switch to it --------------------------------
    let branch = ops::branch::create::create(
        &api,
        ops::branch::create::BranchCreateArgs {
            branch: "feature/itest".into(),
            category: String::new(),
            id: String::new(),
        },
    )
    .await
    .expect("branch::create should succeed");
    assert_eq!(
        branch.name, "feature/itest",
        "branch::create returned an unexpected name: {branch:?}"
    );

    let switched = ops::branch::switch::switch(
        &api,
        ops::branch::switch::BranchSwitchArgs {
            branch: "feature/itest".into(),
            revision: String::new(),
            reset: false,
            bare: false,
        },
    )
    .await
    .expect("branch::switch should succeed");
    assert_eq!(
        switched.branch, "feature/itest",
        "branch::switch returned an unexpected branch: {switched:?}"
    );

    // ---- 6. history should contain the commit we made -----------------------
    let history = ops::revision::history::history(
        &api,
        ops::revision::history::RevisionHistoryArgs::default(),
    )
    .await
    .expect("revision::history should succeed");
    assert!(
        history
            .entries
            .iter()
            .any(|e| e.revision == committed_revision),
        "history did not contain the committed revision {committed_revision}: {history:?}"
    );

    // ---- 7. per-revision info should surface the commit message -------------
    // This is the enrichment path `ClientBackend::log()` uses to populate the
    // main-view history with real commit messages (SBAI-4053). The history op
    // itself carries no message, so we fetch it via `revision::info`.
    let info = ops::revision::info::info(
        &api,
        ops::revision::info::RevisionInfoArgs {
            revision: committed_revision.clone(),
            delta: false,
            metadata: true,
        },
    )
    .await
    .expect("revision::info should succeed");
    assert_eq!(
        info.message(),
        Some("initial commit"),
        "revision::info did not surface the commit message: {info:?}"
    );
}

/// Partition used by the onboarding storage connectivity check. Mirrors the
/// `ONBOARDING_PARTITION` constant in `src-tauri/src/commands.rs`.
const ONBOARDING_PARTITION: &str = "00000000000000000000000000000001";

/// Server-install / onboarding path E2E: the storage round-trip that
/// `ValidateConnectivity` performs in the GUI (open → put → get → obliterate),
/// driven through the same `lore-vm` ops the Tauri commands call. Proves the
/// content-addressed store works headlessly in in-memory mode and that a
/// put/get round-trips the exact bytes.
#[tokio::test]
async fn storage_roundtrip_against_real_lore() {
    let tempdir = tempfile::tempdir().expect("create temp dir");
    let api = in_memory_api(tempdir.path());

    // ---- 1. open an in-memory store -----------------------------------------
    let opened = ops::storage::open::open(
        &api,
        ops::storage::open::StorageOpenArgs {
            repository_path: String::new(),
            in_memory: true,
            remote_url: String::new(),
            cache_target_bytes: 0,
            cache_target_fragments: 0,
        },
    )
    .await
    .expect("storage::open should succeed in memory");
    let handle = opened.handle;

    // ---- 2. put a fragment and capture its content address ------------------
    let payload = b"loregui connectivity check".to_vec();
    let put = ops::storage::put::put(
        &api,
        ops::storage::put::StoragePutArgs {
            handle,
            items: vec![ops::storage::put::PutItem {
                id: 0,
                partition: ONBOARDING_PARTITION.to_string(),
                context: String::new(),
                data: payload.clone(),
                remote_write: false,
                local_cache: false,
                fixed_size_chunk: 0,
            }],
        },
    )
    .await
    .expect("storage::put should succeed");
    let item = put.items.first().expect("put returned no items");
    assert!(item.ok, "put item reported an error: {item:?}");
    let address = item.address.clone();
    assert!(
        !address.is_empty(),
        "put returned an empty address: {item:?}"
    );

    // ---- 3. get it back and assert the bytes round-trip ---------------------
    let got = ops::storage::get::get(
        &api,
        ops::storage::get::StorageGetArgs {
            handle,
            items: vec![ops::storage::get::GetItem {
                id: 0,
                partition: ONBOARDING_PARTITION.to_string(),
                address: address.clone(),
                streaming: false,
                local_cache: false,
            }],
        },
    )
    .await
    .expect("storage::get should succeed");
    let got_item = got.items.first().expect("get returned no items");
    assert!(got_item.ok, "get item reported an error: {got_item:?}");
    assert_eq!(
        got_item.data, payload,
        "storage round-trip mismatch: wrote {payload:?}, read {:?}",
        got_item.data
    );

    // ---- 4. obliterate the test fragment (cleanup the GUI performs) ---------
    let obl = ops::storage::obliterate::obliterate(
        &api,
        ops::storage::obliterate::StorageObliterateArgs {
            handle,
            items: vec![ops::storage::obliterate::ObliterateItem {
                id: 0,
                partition: ONBOARDING_PARTITION.to_string(),
                address: address.clone(),
            }],
        },
    )
    .await
    .expect("storage::obliterate should succeed");
    assert!(
        obl.items.first().map(|i| i.ok).unwrap_or(false),
        "obliterate did not report success: {obl:?}"
    );
}

/// Onboarding "Initialize Server" step E2E: creating a shared store on disk,
/// the first action the host-mode wizard performs before creating a repository.
#[tokio::test]
async fn shared_store_create_against_real_lore() {
    let tempdir = tempfile::tempdir().expect("create temp dir");
    let store_path = tempdir.path().join("shared-store");
    let api = in_memory_api(tempdir.path());

    let created = ops::shared_store::create::create(
        &api,
        ops::shared_store::create::SharedStoreCreateArgs {
            remote_url: String::new(),
            path: Some(store_path.to_string_lossy().into_owned()),
            make_default: false,
        },
    )
    .await
    .expect("shared_store::create should succeed");
    assert!(
        !created.path.is_empty(),
        "shared_store::create returned an empty path: {created:?}"
    );
}
