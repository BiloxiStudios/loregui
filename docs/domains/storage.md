# Domain guide: storage + shared_store

Expert reference for the storage domain. Paired with `loregui-storage-expert`.

## Ops

**storage** (11): `open close flush put put_file get get_file get_metadata copy
obliterate upload`.
**shared_store** (3): `create info set_use_automatically`.

## Model

- **Content-addressed.** `open(config)` → **handle**. `put(handle, items)` hashes
  each buffer → returns its **address**. `get`/`obliterate`/`copy` address fragments
  by `(partition, address)`. The GUI bridges user keys → `(partition,address)` in
  `AppState.storage_session`.
- **Partition** = 32-hex namespace. **Zero partition is rejected** — use a non-zero
  one (`ONBOARDING_PARTITION = "0…01"`).
- **Backends:** `local` (packfiles path) | `s3`/`minio`/`garage` (endpoint, bucket,
  region, access key, secret). Separate **mutable KV store** for branch pointers.
  `StorageBackendConfig` (see `api.ts`) carries all fields.
- **Shared store:** `create(path)` → store path; `info`; `set_use_automatically`.
  Host setup creates a shared store before repositories.
- **FFI gotcha:** `LoreBytes` is a borrowed view; build put items by direct struct
  construction, not serde (see `ops/storage/put.rs`).

## UI (per IA)

- **Storage panel** (sidebar, daily): current backend + connection status; a
  connectivity test (open→put→get→obliterate round-trip, pass/fail + real error);
  fragment/usage info (`get_metadata`); flush.
- **Onboarding host flow:** `BackendPicker` → `ValidateConnectivity` → `InitStore`
  (shared store + first repo) → `ServiceSetup`. Already built; the Storage panel
  reuses `BackendPicker`'s config shape.
- **Palette-only / power-user:** `put put_file get get_file copy obliterate upload
  close get_metadata`. `shared_store_*` → Storage panel / Settings.

## States & safety

Mask secret inputs (access keys); never log them. Connectivity test must use a
real round-trip and surface the actual error. `obliterate` is destructive — confirm.
Empty state: "No storage backend configured — choose one." Loading/error per
DESIGN-SYSTEM.

## Surfaces map

| op | surface |
|---|---|
| open, close, flush, get_metadata | Storage panel + palette |
| put, put_file, get, get_file, copy, obliterate, upload | palette-only |
| shared_store create/info/set_use_automatically | Storage panel/Settings + palette |
