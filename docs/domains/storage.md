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
- **Backends (lore has exactly two):** `local` (filesystem store) and `s3`
  (S3-compatible object store — lore's `aws` store mode). **AWS S3, MinIO, Garage,
  Ceph/RGW, Backblaze B2, … are all the same `s3` backend**, differing only by
  endpoint URL and whether path-style addressing is required — so the picker
  offers them as non-binding *presets*, not separate backends. The `s3` fields are
  endpoint, bucket, region, access key, secret. Separate **mutable KV store** for
  branch pointers. `StorageBackendConfig` (see `api.ts`) carries all fields.
- **Hosting with S3 (`server_host.rs`):** the picker's choice drives the hosted
  `loreserver` config. `local` → `[immutable_store.local]` + `[mutable_store.local]`.
  `s3` → `immutable_store.mode = "aws"` + `[plugins.aws.immutable_store]` (S3 keys:
  `s3_bucket`/`s3_endpoint_url`/`s3_region`/`s3_force_path_style`, plus
  auto-ensured DynamoDB `*_fragments`/`*_fragment-metadata` tables — lore's `aws`
  immutable store pairs S3 payloads with DynamoDB metadata; there is no S3-only
  variant). The **mutable store stays local** (lore's `aws` mutable store needs a
  dedicated DynamoDB table the wizard doesn't provision). Credentials are NOT
  written into the TOML — they're exported to the server process as
  `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` / `AWS_REGION` (lore resolves them
  via the standard AWS credential chain).
- **Advanced / enterprise store modes (deferred — not wired by the wizard):** lore
  also supports a `composite` immutable store (local cache tier + durable `aws`/S3
  tier with a `ReplicationMode` of read/write/read_write), a `replicated`
  server-to-server store (QUIC + mTLS), and `aws`-mode (DynamoDB) **mutable** + a
  DynamoDB **lock** store at scale. These need extra inputs (replica peers,
  DynamoDB tables/region, replication certs) the first-run host wizard doesn't
  collect. To add them: extend `ResolvedConfig` + `render_config_toml` in
  `server_host.rs` with new variants (the local/aws split is already structured
  for it).
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
