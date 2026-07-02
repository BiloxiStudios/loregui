//! Behavioral integration tests for the `storage` op layer against a REAL,
//! disk-backed content-addressed store (SBAI storage-coverage work).
//!
//! The existing per-op unit tests only prove arg/result *serialisation*, and
//! `integration_roundtrip::storage_roundtrip_against_real_lore` exercises a
//! single in-memory `put → get → obliterate` happy path. This suite goes
//! further: it drives the **file-oriented** storage ops (`put_file`,
//! `get_file`, `get_metadata`, `flush`, `close`) against an **on-disk** store
//! and asserts on real behaviour — byte-for-byte content integrity, the
//! `(partition, context)` hex addressing contract, fragment metadata, and the
//! large-file chunked-fragment path.
//!
//! Disk-backed (not in-memory) is the point: it is the closest stand-in for a
//! self-hosted user's `.urc` store and a stronger guard against on-disk
//! persistence/addressing regressions. The store is opened against a REAL repo
//! created with `repository::create` + a shared store living OUTSIDE the
//! working tree (mirroring `e2e_lifecycle`).
//!
//! Gated behind the `integration-tests` cargo feature exactly like
//! `e2e_lifecycle.rs` and `integration_roundtrip.rs`:
//!
//! ```sh
//! cargo test -p lore-vm --features integration-tests --test storage_disk_roundtrip
//! ```
#![cfg(feature = "integration-tests")]

mod storage_support;

use storage_support::{create_disk_repo, on_disk_api, DiskRepo, PARTITION, PARTITION_TWO};

use lore_vm::ops;

/// Open a disk-backed store handle against `repo`'s working tree. `in_memory`
/// is false so the immutable/mutable stores live on disk under the repo.
async fn open_disk_store(repo: &DiskRepo) -> u64 {
    let opened = ops::storage::open::open(
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
    .expect("storage::open should succeed against an on-disk repo");
    assert!(
        opened.handle != 0,
        "open returned a zero handle: {opened:?}"
    );
    opened.handle
}

/// put_file → get_file → flush → get_metadata round trip on a disk store:
/// asserts content integrity (bytes written out match bytes read back) and
/// that the fragment metadata reports the right content size.
#[tokio::test]
async fn put_file_get_file_flush_metadata_roundtrip_on_disk() {
    let repo = create_disk_repo("storage-disk", "alice").await;
    let handle = open_disk_store(&repo).await;

    // Source file on disk that we store into the content-addressed store.
    const CONTENT: &[u8] = b"disk-backed storage round trip: the quick brown fox\n";
    let src = repo.work.path().join("payload.bin");
    storage_support::write_file(&src, CONTENT);

    // ---- put_file ----------------------------------------------------------
    let put = ops::storage::put_file::put_file(
        &repo.api,
        ops::storage::put_file::StoragePutFileArgs {
            handle,
            items: vec![ops::storage::put_file::PutFileItem {
                id: 7,
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
    .expect("storage::put_file should succeed on disk");
    let put_item = put.items.first().expect("put_file returned no items");
    assert!(put_item.ok, "put_file item reported error: {put_item:?}");
    assert_eq!(put_item.id, 7, "put_file echoed the wrong id: {put_item:?}");
    let address = put_item.address.clone();
    assert!(
        !address.is_empty(),
        "put_file returned an empty address: {put_item:?}"
    );

    // ---- flush (fsync to disk) ---------------------------------------------
    ops::storage::flush::flush(&repo.api, ops::storage::flush::StorageFlushArgs { handle })
        .await
        .expect("storage::flush should succeed on disk");

    // ---- get_metadata ------------------------------------------------------
    // Metadata is fetched without transferring payload bytes; assert the
    // content size matches what we stored.
    let meta = ops::storage::get_metadata::get_metadata(
        &repo.api,
        ops::storage::get_metadata::StorageGetMetadataArgs {
            handle,
            items: vec![ops::storage::get_metadata::GetMetadataItem {
                id: 7,
                partition: PARTITION.to_string(),
                address: address.clone(),
            }],
        },
    )
    .await
    .expect("storage::get_metadata should succeed");
    let meta_item = meta.items.first().expect("get_metadata returned no items");
    assert!(meta_item.ok, "get_metadata reported error: {meta_item:?}");
    let frag = meta_item
        .fragment
        .as_ref()
        .expect("get_metadata should carry a fragment on success");
    assert_eq!(
        frag.size_content as usize,
        CONTENT.len(),
        "fragment content size must equal the stored byte count: {meta_item:?}"
    );

    // ---- get_file (write content back to a fresh path) ---------------------
    let out = repo.work.path().join("restored.bin");
    let got = ops::storage::get_file::storage_get_file(
        &repo.api,
        ops::storage::get_file::StorageGetFileArgs {
            handle,
            items: vec![ops::storage::get_file::GetFileItem {
                id: 7,
                partition: PARTITION.to_string(),
                address: address.clone(),
                path: out.to_string_lossy().into_owned(),
                local_cache: false,
            }],
        },
    )
    .await
    .expect("storage::get_file should succeed on disk");
    let got_item = got.items.first().expect("get_file returned no items");
    assert!(got_item.ok, "get_file item reported error: {got_item:?}");

    // Content integrity: the file get_file wrote must be byte-identical.
    let restored = std::fs::read(&out).expect("read restored file");
    assert_eq!(
        restored, CONTENT,
        "get_file content mismatch: wrote {CONTENT:?}, read {restored:?}"
    );

    // Cleanup the handle.
    ops::storage::close::close(&repo.api, ops::storage::close::StorageCloseArgs { handle })
        .await
        .expect("storage::close should succeed");
}

/// Hex `(partition, context)` addressing contract: storing the SAME bytes under
/// a non-zero dedup context yields a *different* address than the zero-context
/// store, and each address is independently retrievable from its own partition.
#[tokio::test]
async fn partition_and_context_hex_addressing() {
    let repo = create_disk_repo("storage-ctx", "alice").await;
    let handle = open_disk_store(&repo).await;

    const CONTENT: &[u8] = b"same bytes, different context\n";
    let src = repo.work.path().join("ctx.bin");
    storage_support::write_file(&src, CONTENT);

    let non_zero_ctx = "ffffffffffffffffffffffffffffffff";

    // Store the same source file twice: once with zero context (default), once
    // with an explicit non-zero context. Different partitions, distinct ids.
    let put = ops::storage::put_file::put_file(
        &repo.api,
        ops::storage::put_file::StoragePutFileArgs {
            handle,
            items: vec![
                ops::storage::put_file::PutFileItem {
                    id: 1,
                    partition: PARTITION.to_string(),
                    context: String::new(),
                    path: src.to_string_lossy().into_owned(),
                    remote_write: false,
                    local_cache: false,
                    fixed_size_chunk: 0,
                },
                ops::storage::put_file::PutFileItem {
                    id: 2,
                    partition: PARTITION_TWO.to_string(),
                    context: non_zero_ctx.to_string(),
                    path: src.to_string_lossy().into_owned(),
                    remote_write: false,
                    local_cache: false,
                    fixed_size_chunk: 0,
                },
            ],
        },
    )
    .await
    .expect("storage::put_file (two contexts) should succeed");
    assert_eq!(put.items.len(), 2, "expected two put results: {put:?}");

    let zero_ctx_item = put.items.iter().find(|i| i.id == 1).expect("id=1 result");
    let ctx_item = put.items.iter().find(|i| i.id == 2).expect("id=2 result");
    assert!(
        zero_ctx_item.ok && ctx_item.ok,
        "both puts must succeed: {put:?}"
    );

    // The content hash is the same payload but the full address embeds the
    // context, so the two addresses must differ.
    assert_ne!(
        zero_ctx_item.address, ctx_item.address,
        "non-zero context must produce a distinct address from zero context: {put:?}"
    );

    // Each address resolves under its own partition, returning the same bytes.
    for (id, partition, address) in [
        (1u64, PARTITION, &zero_ctx_item.address),
        (2u64, PARTITION_TWO, &ctx_item.address),
    ] {
        let out = repo.work.path().join(format!("ctx-out-{id}.bin"));
        let got = ops::storage::get_file::storage_get_file(
            &repo.api,
            ops::storage::get_file::StorageGetFileArgs {
                handle,
                items: vec![ops::storage::get_file::GetFileItem {
                    id,
                    partition: partition.to_string(),
                    address: address.clone(),
                    path: out.to_string_lossy().into_owned(),
                    local_cache: false,
                }],
            },
        )
        .await
        .expect("storage::get_file should resolve a context-addressed item");
        assert!(
            got.items.first().map(|i| i.ok).unwrap_or(false),
            "get_file for id={id} should succeed: {got:?}"
        );
        let restored = std::fs::read(&out).expect("read ctx-addressed output");
        assert_eq!(
            restored, CONTENT,
            "context-addressed content must round-trip for id={id}"
        );
    }

    ops::storage::close::close(&repo.api, ops::storage::close::StorageCloseArgs { handle })
        .await
        .expect("storage::close should succeed");
}

/// Large-file fragment path: store a payload that exceeds a small
/// `fixed_size_chunk` so the engine splits it into multiple leaf fragments,
/// then prove the chunked content reassembles byte-for-byte via get_file and
/// that the metadata content size matches the original.
#[tokio::test]
async fn large_file_chunked_fragments_roundtrip() {
    let repo = create_disk_repo("storage-large", "alice").await;
    let handle = open_disk_store(&repo).await;

    // ~256 KiB of non-repeating-ish bytes so the chunker has real work.
    let mut content = Vec::with_capacity(256 * 1024);
    for i in 0..(256 * 1024u32) {
        content.push((i.wrapping_mul(2654435761) >> 13) as u8);
    }
    let src = repo.work.path().join("large.bin");
    storage_support::write_file(&src, &content);

    // Force a small leaf fragment size so the file is chunked into many leaves.
    let put = ops::storage::put_file::put_file(
        &repo.api,
        ops::storage::put_file::StoragePutFileArgs {
            handle,
            items: vec![ops::storage::put_file::PutFileItem {
                id: 99,
                partition: PARTITION.to_string(),
                context: String::new(),
                path: src.to_string_lossy().into_owned(),
                remote_write: false,
                local_cache: false,
                fixed_size_chunk: 4096,
            }],
        },
    )
    .await
    .expect("storage::put_file (large) should succeed");
    let put_item = put.items.first().expect("put_file returned no items");
    assert!(put_item.ok, "large put_file reported error: {put_item:?}");
    let address = put_item.address.clone();

    ops::storage::flush::flush(&repo.api, ops::storage::flush::StorageFlushArgs { handle })
        .await
        .expect("flush after large put should succeed");

    // Metadata content size equals the full (reassembled) byte count even though
    // it is split across many leaf fragments on disk.
    let meta = ops::storage::get_metadata::get_metadata(
        &repo.api,
        ops::storage::get_metadata::StorageGetMetadataArgs {
            handle,
            items: vec![ops::storage::get_metadata::GetMetadataItem {
                id: 99,
                partition: PARTITION.to_string(),
                address: address.clone(),
            }],
        },
    )
    .await
    .expect("get_metadata for large file should succeed");
    let frag = meta
        .items
        .first()
        .and_then(|i| i.fragment.as_ref())
        .expect("large file should report fragment metadata");
    assert_eq!(
        frag.size_content as usize,
        content.len(),
        "large-file content size must equal original length: {meta:?}"
    );

    // Reassemble via get_file and assert byte-for-byte equality.
    let out = repo.work.path().join("large-out.bin");
    let got = ops::storage::get_file::storage_get_file(
        &repo.api,
        ops::storage::get_file::StorageGetFileArgs {
            handle,
            items: vec![ops::storage::get_file::GetFileItem {
                id: 99,
                partition: PARTITION.to_string(),
                address,
                path: out.to_string_lossy().into_owned(),
                local_cache: false,
            }],
        },
    )
    .await
    .expect("get_file for large file should succeed");
    assert!(
        got.items.first().map(|i| i.ok).unwrap_or(false),
        "large get_file should succeed: {got:?}"
    );
    let restored = std::fs::read(&out).expect("read reassembled large file");
    assert_eq!(
        restored.len(),
        content.len(),
        "reassembled large file length mismatch"
    );
    assert!(
        restored == content,
        "chunked large file must reassemble byte-for-byte"
    );

    ops::storage::close::close(&repo.api, ops::storage::close::StorageCloseArgs { handle })
        .await
        .expect("storage::close should succeed");

    // Silence the unused-import warning when only this test compiles.
    let _ = on_disk_api;
}
