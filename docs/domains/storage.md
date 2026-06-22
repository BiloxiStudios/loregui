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
- **Full server configuration (Expert mode, SBAI-4075):** the host flow has a
  **Basic ↔ Expert** toggle. Basic is port + store + S3 (unchanged, and still
  renders the exact working local config). Expert (`AdvancedServerConfig.tsx`)
  exposes every lore-server `Settings` option grouped into collapsible sections —
  Network (bind host, QUIC, gRPC, HTTP), Storage (local-store flush/eviction/
  capacity + lock-store mode), Topology & replication (`none`/`fixed`/
  `rotating_id_fixed` + peers + the opt-in mTLS `quic_internal`/`replication`
  endpoints), Telemetry, Runtime (Tokio), Notifications, Features, and Shutdown
  timeouts. Each field shows its **lore default** as placeholder/help and is
  **optional**: an omitted field falls through to lore's compiled-in default, so
  `render_config_toml` (in `server_host.rs`) only emits non-default keys. A
  **"View generated config"** button calls `host_server_render_config` to render
  the TOML **without** writing to disk or starting a server, surfacing validation
  errors (bad enum / out-of-range / required-when-mode) as a dry run.
- **Still deferred (need plugin/nested config the wizard doesn't collect):** the
  `composite` immutable store (local cache tier + durable `aws`/S3 tier with a
  `ReplicationMode`), a full `replicated` store with replica peers + a replica
  factory, `aws`-mode (DynamoDB) **mutable** + DynamoDB **lock** store, and the
  `consul`/`composite` topology providers. To add them: extend `*Options` +
  `render_config_toml` in `server_host.rs` (the section structure is already set
  up for it).
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
