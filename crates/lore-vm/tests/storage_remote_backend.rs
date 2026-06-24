//! Behavioral integration coverage for the storage layer's MULTI-BACKEND
//! (local + remote) surface, to the extent CI can drive it headlessly.
//!
//! A genuine second backend (a remote content store) requires a live QUIC
//! `lore` server with TLS material — the same deferred gap `e2e_lifecycle.rs`
//! documents for `branch::push` (`NoRemote` offline). There is no in-process
//! mock remote exposed by the upstream crate, so a true two-backend transfer is
//! NOT runnable in CI here. Rather than skip the surface entirely, this suite:
//!
//!   1. Exercises the remote-config WIRING on `storage::open`: opening with a
//!      `remote_url` is accepted and yields a usable handle whose local side
//!      still round-trips (the remote is configured but unreachable offline).
//!   2. Exercises the `upload` op (the local→remote push trait method) against
//!      a store with NO remote configured, asserting it surfaces a typed
//!      outcome rather than panicking — covering the trait + the offline/no-
//!      remote branch.
//!   3. Asserts the `obliterate` result's remote-side reporting fields behave
//!      correctly when no remote is configured (remote skipped, local success).
//!
//! The genuine networked path (open with a reachable remote → put local →
//! upload → obliterate-remote) is documented here as a deferred gap; closing it
//! needs the same TLS+server harness as the networked server↔client flow.
//!
//! Gated behind the `integration-tests` feature like `e2e_lifecycle.rs`:
//!
//! ```sh
//! cargo test -p lore-vm --features integration-tests --test storage_remote_backend
//! ```
#![cfg(feature = "integration-tests")]

mod storage_support;

use storage_support::{create_disk_repo, write_file, DiskRepo, PARTITION};

use lore_vm::ops;

/// Open a disk-backed handle, optionally with a remote URL configured.
async fn open_with_remote(repo: &DiskRepo, remote_url: &str) -> u64 {
    ops::storage::open::open(
        &repo.api,
        ops::storage::open::StorageOpenArgs {
            repository_path: repo.repo_path.to_string_lossy().into_owned(),
            in_memory: false,
            remote_url: remote_url.to_string(),
            cache_target_bytes: 0,
            cache_target_fragments: 0,
        },
    )
    .await
    .expect("storage::open should succeed")
    .handle
}

/// Opening a store WITH a remote configured is accepted and the local side
/// still round-trips offline — the remote is wired but never contacted.
#[tokio::test]
async fn open_with_remote_config_local_side_still_roundtrips() {
    let repo = create_disk_repo("storage-remote-open", "alice").await;

    // A plausible-looking but unreachable remote endpoint. Offline, the engine
    // must not block on it for purely-local ops.
    let handle = open_with_remote(&repo, "lore://localhost:7777/remote-store").await;
    assert!(handle != 0, "open with remote should yield a non-zero handle");

    const CONTENT: &[u8] = b"local op against a remote-configured store\n";
    let src = repo.work.path().join("remote-cfg.bin");
    write_file(&src, CONTENT);

    // remote_write stays false: this is a purely local put/get, which must
    // succeed even though a remote is configured but unreachable.
    let put = ops::storage::put_file::put_file(
        &repo.api,
        ops::storage::put_file::StoragePutFileArgs {
            handle,
            items: vec![ops::storage::put_file::PutFileItem {
                id: 1,
                partition: PARTITION.to_string(),
                context: String::new(),
                path: src.to_string_lossy().into_owned(),
                remote_write: false,
                local_cache: true,
                fixed_size_chunk: 0,
            }],
        },
    )
    .await
    .expect("local put against a remote-configured store should succeed");
    let addr = put
        .items
        .first()
        .filter(|i| i.ok)
        .map(|i| i.address.clone())
        .expect("local put should produce an address");

    let out = repo.work.path().join("remote-cfg-out.bin");
    let got = ops::storage::get_file::storage_get_file(
        &repo.api,
        ops::storage::get_file::StorageGetFileArgs {
            handle,
            items: vec![ops::storage::get_file::GetFileItem {
                id: 1,
                partition: PARTITION.to_string(),
                address: addr,
                path: out.to_string_lossy().into_owned(),
                local_cache: false,
            }],
        },
    )
    .await
    .expect("local get against a remote-configured store should succeed");
    assert!(
        got.items.first().map(|i| i.ok).unwrap_or(false),
        "local get should round-trip offline: {got:?}"
    );
    assert_eq!(
        std::fs::read(&out).expect("read local round-tripped content"),
        CONTENT,
        "local content must round-trip even with a remote configured"
    );

    ops::storage::close::close(
        &repo.api,
        ops::storage::close::StorageCloseArgs { handle },
    )
    .await
    .expect("close should succeed");
}

/// `upload` (the local→remote push trait method) against a store with NO remote
/// configured must surface a typed outcome rather than panic. We don't assert a
/// specific success/failure code — only that the binding returns cleanly and,
/// if it returns Ok, the per-item result is well-formed. This covers the trait
/// surface and the no-remote branch without a live server.
#[tokio::test]
async fn upload_without_remote_surfaces_typed_outcome() {
    let repo = create_disk_repo("storage-upload", "alice").await;

    // Open WITHOUT a remote.
    let handle = ops::storage::open::open(
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
    .expect("open should succeed")
    .handle;

    // Store something locally to have a real address to attempt uploading.
    let src = repo.work.path().join("to-upload.bin");
    write_file(&src, b"bytes that have no remote to go to\n");
    let put = ops::storage::put_file::put_file(
        &repo.api,
        ops::storage::put_file::StoragePutFileArgs {
            handle,
            items: vec![ops::storage::put_file::PutFileItem {
                id: 1,
                partition: PARTITION.to_string(),
                context: String::new(),
                path: src.to_string_lossy().into_owned(),
                remote_write: false,
                local_cache: false,
                fixed_size_chunk: 0,
            }],
        },
    )
    .await
    .expect("put should succeed");
    let addr = put
        .items
        .first()
        .filter(|i| i.ok)
        .map(|i| i.address.clone())
        .expect("put should produce an address");

    // Attempt upload. With no remote, this either errors at the op level or
    // returns a per-item failure — both are acceptable; a panic / hang is not.
    let result = ops::storage::upload::upload(
        &repo.api,
        ops::storage::upload::StorageUploadArgs {
            handle,
            items: vec![ops::storage::upload::UploadItem {
                id: 1,
                partition: PARTITION.to_string(),
                address: addr,
            }],
        },
    )
    .await;

    match result {
        Ok(res) => {
            // If the op returns Ok, the per-item result must be well-formed:
            // either it reports not-ok (no remote) or already_durable. We don't
            // require a specific code, just internal consistency.
            if let Some(item) = res.items.first() {
                if item.ok {
                    // A spurious "ok" with no remote would still be well-formed
                    // (already_durable path); just assert no error text leaked.
                    assert!(
                        item.error.is_empty(),
                        "ok upload item must not carry an error string: {item:?}"
                    );
                } else {
                    assert!(
                        !item.error.is_empty(),
                        "a failed upload item should carry an error code: {item:?}"
                    );
                }
            }
        }
        Err(_) => {
            // Acceptable: no remote configured surfaces as an op-level error.
        }
    }

    ops::storage::close::close(
        &repo.api,
        ops::storage::close::StorageCloseArgs { handle },
    )
    .await
    .expect("close should succeed");
}

/// `obliterate` with no remote configured reports the remote side as skipped
/// while the local side succeeds — covering the multi-side result reporting
/// fields that only matter once a remote is in play.
#[tokio::test]
async fn obliterate_reports_remote_skipped_without_remote() {
    let repo = create_disk_repo("storage-obl-remote", "alice").await;
    let handle = ops::storage::open::open(
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
    .expect("open should succeed")
    .handle;

    let src = repo.work.path().join("obl-remote.bin");
    write_file(&src, b"local-only fragment\n");
    let put = ops::storage::put_file::put_file(
        &repo.api,
        ops::storage::put_file::StoragePutFileArgs {
            handle,
            items: vec![ops::storage::put_file::PutFileItem {
                id: 1,
                partition: PARTITION.to_string(),
                context: String::new(),
                path: src.to_string_lossy().into_owned(),
                remote_write: false,
                local_cache: false,
                fixed_size_chunk: 0,
            }],
        },
    )
    .await
    .expect("put should succeed");
    let addr = put
        .items
        .first()
        .filter(|i| i.ok)
        .map(|i| i.address.clone())
        .expect("put should produce an address");

    let obl = ops::storage::obliterate::obliterate(
        &repo.api,
        ops::storage::obliterate::StorageObliterateArgs {
            handle,
            items: vec![ops::storage::obliterate::ObliterateItem {
                id: 1,
                partition: PARTITION.to_string(),
                address: addr,
            }],
        },
    )
    .await
    .expect("obliterate should succeed");
    let item = obl.items.first().expect("obliterate returned no items");
    assert!(item.ok, "obliterate should succeed: {item:?}");
    assert!(
        item.local_success,
        "local side should report success: {item:?}"
    );
    assert!(
        item.remote_skipped,
        "with no remote configured, the remote side must be skipped: {item:?}"
    );

    ops::storage::close::close(
        &repo.api,
        ops::storage::close::StorageCloseArgs { handle },
    )
    .await
    .expect("close should succeed");
}
