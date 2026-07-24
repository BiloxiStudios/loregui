# ADR-0003: Mutable Storage Ops — Binding Assessment (SBAI-5473)

**Date:** 2026-07-21
**Status:** Accepted — needs upstream work
**Context:** SBAI-5473 parity scan reports 4 unbound nightly storage ops

## The Four Ops

The upstream `lore-storage` crate defines a `MutableStore` trait with four methods:

| Method | Purpose |
|--------|---------|
| `load(partition, key, key_type)` | Read a mutable KV value (returns a Hash) |
| `store(partition, key, value, key_type)` | Write a mutable KV value |
| `compare_and_swap(partition, key, expected, value, key_type)` | Atomic CAS on a mutable KV value |
| `list(partition, key_type)` | Enumerate all key-value pairs of a type |

These are **low-level storage primitives** used internally by the lore engine for
branch pointers, staged-anchor bookkeeping, and internal state management. They
operate on `Hash` values (content addresses) and `Partition`/`KeyType` enums.

## Why They Cannot Be Bound (Yet)

lore-vm ops bind the `lore` crate's **C-FFI event-callback layer**:
`lore::interface::*` types and `extern "C"` functions that deliver results via
`LoreEventCallback`. This is the canonical seam for all ~140 ops.

The `MutableStore` trait is a **pure Rust trait** in the `lore-storage` crate.
It is NOT exposed through the `lore` crate's C-FFI layer:

- No `lore::storage::mutable_load()` etc. functions exist
- No `LoreEvent::MutableStore*` event variants exist
- The `lore` crate does not re-export `lore_storage::MutableStore`

lore-vm depends on `lore` (workspace), not `lore-storage` directly. Even if it
did, lore-vm's op architecture expects the event-callback pattern, not direct
trait calls.

## What's Needed Upstream

To bind these ops in lore-vm, the `lore` crate needs:

1. **C-FFI wrapper functions**: `lore_mutable_store_load()`, etc., accepting
   serialized args and delivering results via the existing event-callback mechanism.
2. **New event types**: `LORE_EVENT_MUTABLE_STORE_*` variants in `LoreEvent`.
3. **Public re-export or internal routing**: Either expose `MutableStore` from
   `lore::storage::mutable` or route through the engine's internal handle registry.

## Interim Approach

These ops are **internal bookkeeping primitives**, not user-facing operations.
The LoreGUI does not currently expose any UI for direct mutable store access.
The onboarding storage flow (`storage_open`/`storage_put`/`storage_get`) works
at the content-addressed level, not the mutable KV level.

Until upstream exposes them, these 4 ops remain unbound. The parity scanner
correctly identifies them as gaps — they are real upstream operations that
have no LoreGUI binding. No action is required at the LoreGUI layer until
the lore crate adds the C-FFI surface.

## lock.file_message_send — False Positive

The parity scanner also reports `lock.file_message_send` as orphaned. This is
**incorrect**: the op exists in upstream lore (`lore_lock_file_message_send` in
`lore/src/interface.rs`), IS in lore-vm dispatch.rs `SUPPORTED_OPS`, and IS
routed by the dispatch match. The scanner's classification is a false positive.

## revision.activity_report — Resolved

The `revision.activity_report` op implementation exists (`ops/revision/activity_report.rs`)
with a Tauri command, but was NOT wired into dispatch.rs `SUPPORTED_OPS`. This
has been fixed: the op is now in SUPPORTED_OPS and routed by dispatch, making it
reachable via CLI/FFI in addition to the existing Tauri path.

This op is a LoreGUI-native composite (synthesizes a report from
`revision.history` + `revision.info`), not an upstream Lore operation. The parity
scanner's "orphan" report for this op was correct before this fix — the op existed
in lore-vm but wasn't dispatch-routable.
