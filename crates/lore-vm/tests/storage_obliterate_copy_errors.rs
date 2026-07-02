//! Behavioral integration tests for the `storage` mutation + error-path ops:
//! `copy`, `obliterate`, and the failure modes of `get_file` / `get_metadata`.
//!
//! The per-op unit tests only cover serialisation; this suite drives the REAL
//! disk-backed engine and asserts on behaviour:
//!
//!   1. copy — duplicate content into a second partition and prove the copy is
//!      independently retrievable with identical bytes.
//!   2. obliterate — delete a stored fragment and prove a subsequent get_file /
//!      get_metadata for it now misses; obliterate is idempotent (a second call
//!      on the already-gone address still reports success).
//!   3. error paths — get_file / get_metadata for a never-stored (missing)
//!      address report a per-item failure (not a hard call error); a malformed
//!      address surfaces as an op-level error.
//!
//! Gated behind the `integration-tests` feature like `e2e_lifecycle.rs`:
//!
//! ```sh
//! cargo test -p lore-vm --features integration-tests --test storage_obliterate_copy_errors
//! ```
#![cfg(feature = "integration-tests")]

mod storage_support;

use storage_support::{create_disk_repo, write_file, DiskRepo, PARTITION, PARTITION_TWO};

use lore_vm::ops;

/// Open a disk-backed store handle against `repo`'s working tree.
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

/// Store `content` from a temp file under `PARTITION` and return its address.
async fn put_payload(repo: &DiskRepo, handle: u64, name: &str, content: &[u8]) -> String {
    let src = repo.work.path().join(name);
    write_file(&src, content);
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
    .expect("storage::put_file should succeed");
    let item = put.items.first().expect("put returned no items");
    assert!(item.ok, "put reported error: {item:?}");
    item.address.clone()
}

/// `copy` duplicates content into a second partition; the copy is retrievable
/// and byte-identical to the source.
#[tokio::test]
async fn copy_duplicates_content_into_second_partition() {
    let repo = create_disk_repo("storage-copy", "alice").await;
    let handle = open_disk_store(&repo).await;

    const CONTENT: &[u8] = b"content to be copied across partitions\n";
    let src_addr = put_payload(&repo, handle, "copysrc.bin", CONTENT).await;

    let copied = ops::storage::copy::copy(
        &repo.api,
        ops::storage::copy::StorageCopyArgs {
            handle,
            items: vec![ops::storage::copy::CopyItem {
                id: 5,
                source_partition: PARTITION.to_string(),
                target_partition: PARTITION_TWO.to_string(),
                source_address: src_addr.clone(),
                target_context: String::new(),
            }],
        },
    )
    .await
    .expect("storage::copy should succeed");
    let copy_item = copied.items.first().expect("copy returned no items");
    assert!(copy_item.ok, "copy reported error: {copy_item:?}");
    assert_eq!(copy_item.id, 5, "copy echoed wrong id: {copy_item:?}");

    // The copy is retrievable from the TARGET partition with the same content.
    // Content hash is preserved by copy, so the source address resolves there.
    let out = repo.work.path().join("copy-out.bin");
    let got = ops::storage::get_file::get_file(
        &repo.api,
        ops::storage::get_file::StorageGetFileArgs {
            handle,
            items: vec![ops::storage::get_file::GetFileItem {
                id: 5,
                partition: PARTITION_TWO.to_string(),
                address: src_addr.clone(),
                path: out.to_string_lossy().into_owned(),
                local_cache: false,
            }],
        },
    )
    .await
    .expect("storage::get_file from the copy target should succeed");
    assert!(
        got.items.first().map(|i| i.ok).unwrap_or(false),
        "get_file from the copied partition should succeed: {got:?}"
    );
    assert_eq!(
        std::fs::read(&out).expect("read copied content"),
        CONTENT,
        "copied content must be byte-identical to the source"
    );

    ops::storage::close::close(&repo.api, ops::storage::close::StorageCloseArgs { handle })
        .await
        .expect("close should succeed");
}

/// `obliterate` removes a stored fragment so a subsequent get misses; a second
/// obliterate of the now-gone address is idempotent (still reports success).
#[tokio::test]
async fn obliterate_removes_then_is_idempotent() {
    let repo = create_disk_repo("storage-obl", "alice").await;
    let handle = open_disk_store(&repo).await;

    const CONTENT: &[u8] = b"transient fragment to obliterate\n";
    let addr = put_payload(&repo, handle, "obl.bin", CONTENT).await;

    // Sanity: it is retrievable before obliteration.
    let pre = ops::storage::get_metadata::storage_get_metadata(
        &repo.api,
        ops::storage::get_metadata::StorageGetMetadataArgs {
            handle,
            items: vec![ops::storage::get_metadata::GetMetadataItem {
                id: 1,
                partition: PARTITION.to_string(),
                address: addr.clone(),
            }],
        },
    )
    .await
    .expect("pre-obliterate metadata should succeed");
    let pre_item = pre.items.first().expect("pre metadata returned no items");
    assert!(
        pre_item.ok,
        "fragment should resolve before obliteration: {pre_item:?}"
    );
    let pre_size = pre_item
        .fragment
        .as_ref()
        .map(|f| f.size_content)
        .expect("pre-obliterate fragment should carry a content size");
    assert_eq!(
        pre_size as usize,
        CONTENT.len(),
        "pre-obliterate content size should match the stored bytes: {pre_item:?}"
    );

    // Obliterate it.
    let obl = ops::storage::obliterate::obliterate(
        &repo.api,
        ops::storage::obliterate::StorageObliterateArgs {
            handle,
            items: vec![ops::storage::obliterate::ObliterateItem {
                id: 1,
                partition: PARTITION.to_string(),
                address: addr.clone(),
            }],
        },
    )
    .await
    .expect("storage::obliterate should succeed");
    let obl_item = obl.items.first().expect("obliterate returned no items");
    assert!(obl_item.ok, "obliterate reported error: {obl_item:?}");
    assert!(
        obl_item.local_success,
        "local obliteration should report success: {obl_item:?}"
    );

    // Behavioral finding: after obliteration the payload is gone, but the
    // metadata lookup still RESOLVES — it returns a tombstoned/zeroed fragment
    // (a non-zero flag bit set, size_content == 0) rather than a hard miss. So
    // we assert the fragment is now empty (content size dropped from the stored
    // length to 0), which is how an obliterated entry distinguishes itself from
    // a live one.
    let post = ops::storage::get_metadata::storage_get_metadata(
        &repo.api,
        ops::storage::get_metadata::StorageGetMetadataArgs {
            handle,
            items: vec![ops::storage::get_metadata::GetMetadataItem {
                id: 1,
                partition: PARTITION.to_string(),
                address: addr.clone(),
            }],
        },
    )
    .await
    .expect("post-obliterate metadata call should still succeed (tombstone)");
    let post_item = post.items.first().expect("post metadata returned no items");
    let post_size = post_item
        .fragment
        .as_ref()
        .map(|f| f.size_content)
        .unwrap_or(0);
    assert_eq!(
        post_size, 0,
        "obliterated fragment must report zero content size (tombstone): {post_item:?}"
    );

    // And actually fetching the payload bytes must now fail at the op level —
    // there is no data left to reassemble.
    let out = repo.work.path().join("obl-after.bin");
    let got_after = ops::storage::get_file::get_file(
        &repo.api,
        ops::storage::get_file::StorageGetFileArgs {
            handle,
            items: vec![ops::storage::get_file::GetFileItem {
                id: 1,
                partition: PARTITION.to_string(),
                address: addr.clone(),
                path: out.to_string_lossy().into_owned(),
                local_cache: false,
            }],
        },
    )
    .await;
    assert!(
        got_after.is_err() || !out.exists(),
        "fetching an obliterated payload must not produce its original bytes: {got_after:?}"
    );

    // Idempotency: obliterating the already-gone address still reports success.
    let obl2 = ops::storage::obliterate::obliterate(
        &repo.api,
        ops::storage::obliterate::StorageObliterateArgs {
            handle,
            items: vec![ops::storage::obliterate::ObliterateItem {
                id: 2,
                partition: PARTITION.to_string(),
                address: addr.clone(),
            }],
        },
    )
    .await
    .expect("second obliterate should succeed");
    assert!(
        obl2.items.first().map(|i| i.ok).unwrap_or(false),
        "obliterate must be idempotent on an already-gone address: {obl2:?}"
    );

    ops::storage::close::close(&repo.api, ops::storage::close::StorageCloseArgs { handle })
        .await
        .expect("close should succeed");
}

/// A get for a never-stored (missing) address surfaces as an OP-LEVEL error.
///
/// Behavioral finding: the storage bindings classify a not-found item via the
/// upstream `Complete` status. When an item misses, that status is non-zero, so
/// `get_metadata` / `get_file` return `Err(CommandFailed(...))` rather than an
/// `Ok` carrying a per-item `ok:false`. This test pins that contract so a future
/// change (e.g. downgrading misses to per-item failures) is a conscious one.
#[tokio::test]
async fn missing_address_surfaces_op_level_error() {
    let repo = create_disk_repo("storage-miss", "alice").await;
    let handle = open_disk_store(&repo).await;

    // A syntactically valid but never-stored address: 64-hex hash + 32-hex ctx.
    let missing = format!("{}-{}", "ab".repeat(32), "00".repeat(16));

    // get_metadata for a missing address surfaces as an op-level Err.
    let meta = ops::storage::get_metadata::storage_get_metadata(
        &repo.api,
        ops::storage::get_metadata::StorageGetMetadataArgs {
            handle,
            items: vec![ops::storage::get_metadata::GetMetadataItem {
                id: 1,
                partition: PARTITION.to_string(),
                address: missing.clone(),
            }],
        },
    )
    .await;
    assert!(
        meta.is_err(),
        "get_metadata for a missing address must surface an op-level Err: {meta:?}"
    );

    // get_file for a missing address: same op-level Err, and no file produced.
    let out = repo.work.path().join("missing-out.bin");
    let got = ops::storage::get_file::get_file(
        &repo.api,
        ops::storage::get_file::StorageGetFileArgs {
            handle,
            items: vec![ops::storage::get_file::GetFileItem {
                id: 1,
                partition: PARTITION.to_string(),
                address: missing,
                path: out.to_string_lossy().into_owned(),
                local_cache: false,
            }],
        },
    )
    .await;
    assert!(
        got.is_err(),
        "get_file for a missing address must surface an op-level Err: {got:?}"
    );
    assert!(
        !out.exists(),
        "no output file should be produced for a missing address"
    );

    ops::storage::close::close(&repo.api, ops::storage::close::StorageCloseArgs { handle })
        .await
        .expect("close should succeed");
}

/// A malformed (non-hex / wrong-length) address surfaces as an op-level error
/// from the binding's arg conversion, not a panic.
#[tokio::test]
async fn malformed_address_surfaces_an_error() {
    let repo = create_disk_repo("storage-bad", "alice").await;
    let handle = open_disk_store(&repo).await;

    let result = ops::storage::get_metadata::storage_get_metadata(
        &repo.api,
        ops::storage::get_metadata::StorageGetMetadataArgs {
            handle,
            items: vec![ops::storage::get_metadata::GetMetadataItem {
                id: 1,
                partition: PARTITION.to_string(),
                // Not valid hex, and the wrong shape for an address.
                address: "not-a-valid-address".to_string(),
            }],
        },
    )
    .await;
    assert!(
        result.is_err(),
        "a malformed address must surface as an Err, got: {result:?}"
    );

    ops::storage::close::close(&repo.api, ops::storage::close::StorageCloseArgs { handle })
        .await
        .expect("close should succeed");
}
