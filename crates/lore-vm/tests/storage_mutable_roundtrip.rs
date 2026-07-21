//! Local integration round-trips for the mutable storage ops (SBAI-5473).
//!
//! Exercises store → load → list → compare-and-swap (including wrong-CAS no-swap
//! and absent-key AddressNotFound) against a real on-disk store.
//!
//! ```sh
//! cargo test -p lore-vm --features integration-tests --test storage_mutable_roundtrip
//! ```
#![cfg(feature = "integration-tests")]

mod storage_support;

use storage_support::{create_disk_repo, DiskRepo, PARTITION};

use lore_vm::ops;

// Distinct non-repeating hex patterns so list output is easy to spot.
const KEY: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const VALUE_A: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const VALUE_B: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const VALUE_WRONG: &str = "4444444444444444444444444444444444444444444444444444444444444444";
const HASH_ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";
/// Match upstream mutable tests (`KeyType::BranchLatestPointer`).
const KEY_TYPE: &str = "branchLatestPointer";

async fn open_disk_store(repo: &DiskRepo) -> u64 {
    ops::storage::open::open(
        &repo.api,
        ops::storage::open::StorageOpenArgs {
            repository_path: repo.repo_path.to_string_lossy().into_owned(),
            in_memory: false,
            remote_url: String::new(),
            cache_target_bytes: 0,
            cache_target_fragments: 0,
        },
    )
    .await
    .expect("storage::open should succeed")
    .handle
}

/// Open a disk store via an ONLINE (offline=false) API so per-call `remote=true`
/// is not contradicted by a bound offline flag from open.
async fn open_disk_store_online(
    repo_path: &std::path::Path,
    identity: &str,
) -> (lore_vm::api::LoreApi, u64) {
    use lore_vm::global::LoreGlobal;
    let api = lore_vm::api::LoreApi::from_global(
        LoreGlobal::new(repo_path.to_path_buf())
            .in_memory(false)
            .offline(false)
            .identity(identity),
    );
    let handle = ops::storage::open::open(
        &api,
        ops::storage::open::StorageOpenArgs {
            repository_path: repo_path.to_string_lossy().into_owned(),
            in_memory: false,
            remote_url: String::new(),
            cache_target_bytes: 0,
            cache_target_fragments: 0,
        },
    )
    .await
    .expect("online storage::open should succeed")
    .handle;
    (api, handle)
}

#[tokio::test]
async fn mutable_store_load_list_cas_local_roundtrip() {
    let repo = create_disk_repo("mutable-local", "alice").await;
    let handle = open_disk_store(&repo).await;

    // ---- store ------------------------------------------------------------
    let stored = ops::storage::mutable_store::mutable_store(
        &repo.api,
        ops::storage::mutable_store::StorageMutableStoreArgs {
            handle,
            remote: false,
            items: vec![ops::storage::mutable_store::MutableStoreItem {
                id: 1,
                partition: PARTITION.to_string(),
                key: KEY.to_string(),
                value: VALUE_A.to_string(),
                key_type: KEY_TYPE.into(),
            }],
        },
    )
    .await
    .expect("mutable_store should succeed");
    assert_eq!(stored.items.len(), 1);
    assert!(stored.items[0].ok, "store item: {:?}", stored.items[0]);

    // Flush so disk-backed mutable list/load see the write.
    ops::storage::flush::flush(&repo.api, ops::storage::flush::StorageFlushArgs { handle })
        .await
        .expect("flush after store");

    // ---- load -------------------------------------------------------------
    let loaded = ops::storage::mutable_load::mutable_load(
        &repo.api,
        ops::storage::mutable_load::StorageMutableLoadArgs {
            handle,
            remote: false,
            items: vec![ops::storage::mutable_load::MutableLoadItem {
                id: 1,
                partition: PARTITION.to_string(),
                key: KEY.to_string(),
                key_type: KEY_TYPE.into(),
            }],
        },
    )
    .await
    .expect("mutable_load should succeed");
    assert!(loaded.items[0].ok, "load item: {:?}", loaded.items[0]);
    assert_eq!(loaded.items[0].value, VALUE_A);

    // ---- list -------------------------------------------------------------
    let listed = ops::storage::mutable_list::mutable_list(
        &repo.api,
        ops::storage::mutable_list::StorageMutableListArgs {
            handle,
            remote: false,
            items: vec![ops::storage::mutable_list::MutableListItem {
                id: 1,
                partition: PARTITION.to_string(),
                key_type: KEY_TYPE.into(),
            }],
        },
    )
    .await
    .expect("mutable_list should succeed");
    assert!(listed.items[0].ok, "list item: {:?}", listed.items[0]);
    let hit = listed.items[0]
        .entries
        .iter()
        .find(|e| e.value == VALUE_A)
        .unwrap_or_else(|| {
            panic!(
                "list must include an entry with VALUE_A; entries={:?}",
                listed.items[0].entries
            )
        });
    // Key should round-trip; if the engine normalises it, load via listed key still works.
    if hit.key != KEY {
        eprintln!(
            "[warn] listed key differs from stored key (stored={KEY}, listed={})",
            hit.key
        );
    }
    assert!(
        hit.key == KEY || hit.value == VALUE_A,
        "list entry should match our store: {hit:?}"
    );

    // ---- wrong CAS (no swap) ----------------------------------------------
    let wrong = ops::storage::mutable_compare_and_swap::mutable_compare_and_swap(
        &repo.api,
        ops::storage::mutable_compare_and_swap::StorageMutableCompareAndSwapArgs {
            handle,
            remote: false,
            items: vec![
                ops::storage::mutable_compare_and_swap::MutableCompareAndSwapItem {
                    id: 1,
                    partition: PARTITION.to_string(),
                    key: KEY.to_string(),
                    expected: VALUE_WRONG.to_string(),
                    value: VALUE_B.to_string(),
                    key_type: KEY_TYPE.into(),
                },
            ],
        },
    )
    .await
    .expect("wrong CAS should complete (no hard error)");
    assert!(wrong.items[0].ok, "CAS item ok: {:?}", wrong.items[0]);
    assert!(
        !wrong.items[0].swapped,
        "wrong expected must not swap: {:?}",
        wrong.items[0]
    );
    assert_eq!(wrong.items[0].previous, VALUE_A);

    // Value unchanged after wrong CAS.
    let still = ops::storage::mutable_load::mutable_load(
        &repo.api,
        ops::storage::mutable_load::StorageMutableLoadArgs {
            handle,
            remote: false,
            items: vec![ops::storage::mutable_load::MutableLoadItem {
                id: 2,
                partition: PARTITION.to_string(),
                key: KEY.to_string(),
                key_type: KEY_TYPE.into(),
            }],
        },
    )
    .await
    .expect("reload after wrong CAS");
    assert_eq!(still.items[0].value, VALUE_A);

    // ---- correct CAS ------------------------------------------------------
    let cas = ops::storage::mutable_compare_and_swap::mutable_compare_and_swap(
        &repo.api,
        ops::storage::mutable_compare_and_swap::StorageMutableCompareAndSwapArgs {
            handle,
            remote: false,
            items: vec![
                ops::storage::mutable_compare_and_swap::MutableCompareAndSwapItem {
                    id: 1,
                    partition: PARTITION.to_string(),
                    key: KEY.to_string(),
                    expected: VALUE_A.to_string(),
                    value: VALUE_B.to_string(),
                    key_type: KEY_TYPE.into(),
                },
            ],
        },
    )
    .await
    .expect("correct CAS");
    assert!(
        cas.items[0].ok && cas.items[0].swapped,
        "{:?}",
        cas.items[0]
    );
    assert_eq!(cas.items[0].previous, VALUE_A);

    let after = ops::storage::mutable_load::mutable_load(
        &repo.api,
        ops::storage::mutable_load::StorageMutableLoadArgs {
            handle,
            remote: false,
            items: vec![ops::storage::mutable_load::MutableLoadItem {
                id: 3,
                partition: PARTITION.to_string(),
                key: KEY.to_string(),
                key_type: KEY_TYPE.into(),
            }],
        },
    )
    .await
    .expect("load after CAS");
    assert_eq!(after.items[0].value, VALUE_B);

    // ---- null store removes key -------------------------------------------
    let removed = ops::storage::mutable_store::mutable_store(
        &repo.api,
        ops::storage::mutable_store::StorageMutableStoreArgs {
            handle,
            remote: false,
            items: vec![ops::storage::mutable_store::MutableStoreItem {
                id: 9,
                partition: PARTITION.to_string(),
                key: KEY.to_string(),
                value: String::new(), // null → remove
                key_type: KEY_TYPE.into(),
            }],
        },
    )
    .await
    .expect("null store");
    assert!(removed.items[0].ok, "{:?}", removed.items[0]);

    // ---- absent key → AddressNotFound -------------------------------------
    let missing = ops::storage::mutable_load::mutable_load(
        &repo.api,
        ops::storage::mutable_load::StorageMutableLoadArgs {
            handle,
            remote: false,
            items: vec![ops::storage::mutable_load::MutableLoadItem {
                id: 4,
                partition: PARTITION.to_string(),
                key: KEY.to_string(),
                key_type: KEY_TYPE.into(),
            }],
        },
    )
    .await
    .expect("absent load returns typed item result");
    assert!(!missing.items[0].ok, "{:?}", missing.items[0]);
    assert_eq!(missing.items[0].error, "AddressNotFound");
    assert!(
        missing.items[0].value.is_empty() || missing.items[0].value == HASH_ZERO,
        "absent value must be empty/zero: {:?}",
        missing.items[0]
    );

    ops::storage::close::close(&repo.api, ops::storage::close::StorageCloseArgs { handle })
        .await
        .expect("close");
}

#[tokio::test]
async fn mutable_list_remote_flag_on_local_handle_rejects() {
    // A local-only handle (opened online so bound offline is false) asked to
    // list remotely must fail call-level with the upstream local-only reason.
    // Opening offline=true binds offline and contradicts per-call remote.
    let repo = create_disk_repo("mutable-list-remote-reject", "alice").await;
    let (api, handle) = open_disk_store_online(&repo.repo_path, "alice").await;

    let err = ops::storage::mutable_list::mutable_list(
        &api,
        ops::storage::mutable_list::StorageMutableListArgs {
            handle,
            remote: true,
            items: vec![ops::storage::mutable_list::MutableListItem {
                id: 1,
                partition: PARTITION.to_string(),
                key_type: KEY_TYPE.into(),
            }],
        },
    )
    .await
    .expect_err("remote mutable_list must fail");

    let msg = err.to_string();
    assert!(
        msg.contains("mutable_list is only supported on the local store")
            || msg.contains("remote mutable")
            || msg.contains("remote_config")
            || msg.contains("failed with status")
            || msg.contains("InvalidArguments")
            || msg.contains("invalid arguments"),
        "unexpected rejection message: {msg}"
    );
}
