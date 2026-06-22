//! Hosting a real `lore` server from the GUI (SBAI-4065).
//!
//! The onboarding "Host a server" flow used to call `service_start`, which maps
//! to `lore::service::start` — an upstream **stub** that returns 1 and hosts
//! nothing. The genuine server is the standalone upstream **`loreserver`**
//! binary (crate `lore-server`, bin `loreserver`), driven entirely by a layered
//! TOML config. The SBAI-4064 spike (`scripts/live-server-client.sh` +
//! `docs/live-server-client-spike.md`) proved the exact recipe; this module
//! productionises it: generate the config, resolve the binary, spawn it as a
//! managed child, and expose start/stop/status.
//!
//! The server binds `127.0.0.1` only, serves the host flow's local immutable +
//! mutable stores, ships the upstream self-signed test certs for QUIC, and runs
//! with **auth disabled** (no `[server.auth]` block) for the local/no-auth case.
//! An `auth` hook is kept on [`HostServerOptions`] for a future authed mode.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use lore_vm::LoreError;
use serde::{Deserialize, Serialize};

/// Default QUIC/gRPC port for a hosted server. The HTTP service is `port + 2`,
/// matching the spike. 41337 is the spike default and is unprivileged.
pub const DEFAULT_PORT: u16 = 41337;

/// Bind host. We host on loopback only — exposing a `lore` server to a LAN/WAN
/// is a deliberate, separate concern (firewalling, real certs, auth) and is not
/// what the first-run "Host a server" flow does.
const BIND_HOST: &str = "127.0.0.1";

/// Inputs from the frontend "Host a server" flow.
///
/// # Defaults handling (SBAI-4075)
///
/// Every advanced section is **optional** and every field within it is
/// `Option<_>`. The renderer ([`render_config_toml`]) only emits a key when the
/// user supplied an explicit, non-default value, so:
///
/// - An all-`None` [`HostServerOptions`] (the simple first-run case) renders the
///   exact same minimal local config it always did — lore fills in every other
///   field from its own compiled-in `default.toml`.
/// - A field the user leaves blank is **omitted**, which means "use lore's
///   default" rather than "force lore's default" — the two are equivalent
///   because lore's `default.toml` is the base layer of its config stack.
///
/// The lore default for each field is documented inline (and surfaced to the UI
/// as placeholder/help text) so the operator always knows what omitting it does.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostServerOptions {
    /// Directory that backs the immutable + mutable stores. This MUST be the
    /// same store the host flow's `shared_store` / `repository` create used, so
    /// the repository the user just created is actually served.
    pub store_dir: String,
    /// QUIC/gRPC port. Defaults to [`DEFAULT_PORT`] when absent/zero.
    #[serde(default)]
    pub port: Option<u16>,
    /// Repository name to embed in the advertised `lore://host:port/<name>` URL
    /// so the success screen can show clients exactly what to clone. Optional —
    /// when absent the URL is the bare `lore://host:port`.
    #[serde(default)]
    pub repository_name: Option<String>,
    /// Reserved hook for a future authed mode. When `true` the generated config
    /// would include a `[server.auth]` block (JWK/issuer). Not yet implemented —
    /// the local host flow is no-auth; accepted for forward-compat.
    #[serde(default)]
    pub auth: bool,
    /// Bind host for every endpoint (QUIC/gRPC/HTTP). Defaults to
    /// [`BIND_HOST`] (`127.0.0.1`) — loopback only. Set to `0.0.0.0` to expose
    /// the server on the LAN/WAN (a deliberate choice: firewalling + real certs
    /// become the operator's responsibility).
    #[serde(default)]
    pub bind_host: Option<String>,
    /// Optional S3-compatible object-storage backing for the **immutable** store
    /// (lore's `aws` store mode). When `None`, the immutable store is a local
    /// filesystem store under [`store_dir`](Self::store_dir).
    ///
    /// The **mutable** (branch-pointer) store stays local in both cases: lore's
    /// `aws` mutable store is backed by DynamoDB, which the host wizard does not
    /// provision. Pairing an S3 immutable store with a local mutable store is a
    /// valid lore topology for a single-node host (cf. the upstream
    /// `composite.local + aws.durable` recipe in `dev-local.toml`).
    #[serde(default)]
    pub s3: Option<S3StoreOptions>,

    /// All advanced (Expert-mode) sections in one bag (SBAI-4075). `None` (the
    /// simple first-run case) means "use lore's defaults for everything", and
    /// renders the original minimal local config exactly.
    #[serde(default)]
    pub advanced: Option<HostAdvancedOptions>,
}

/// The full Expert-mode configuration surface (SBAI-4075): one optional bag of
/// optional sections. Carried as a single nested object so both the typed start
/// call and the `host_server_render_config` preview send it whole, while the
/// flat palette-driven `host_server_start` simply omits it.
///
/// Every field is `Option`/empty; whatever is left unset falls through to lore's
/// own compiled-in default, so a `Default` value adds nothing to the config.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostAdvancedOptions {
    /// QUIC transport tuning ([`server.quic`]).
    #[serde(default)]
    pub quic: Option<QuicOptions>,
    /// gRPC endpoint tuning ([`server.grpc`]).
    #[serde(default)]
    pub grpc: Option<GrpcOptions>,
    /// HTTP endpoint tuning ([`server.http`]).
    #[serde(default)]
    pub http: Option<HttpOptions>,
    /// Local-store tuning (flush/eviction/capacity) for the filesystem stores.
    #[serde(default)]
    pub local_store: Option<LocalStoreOptions>,
    /// Single-node fixed/rotating peer topology + replication ([`topology`]).
    #[serde(default)]
    pub topology: Option<TopologyOptions>,
    /// Telemetry: logger / metrics / traces ([`telemetry`]).
    #[serde(default)]
    pub telemetry: Option<TelemetryOptions>,
    /// Tokio runtime threads ([`tokio`]).
    #[serde(default)]
    pub runtime: Option<RuntimeOptions>,
    /// Notification backend mode ([`notification`]).
    #[serde(default)]
    pub notification: Option<NotificationOptions>,
    /// Revision/history feature flags ([`feature`]).
    #[serde(default)]
    pub features: Option<FeatureOptions>,
    /// Graceful-shutdown timeouts ([`server`]).
    #[serde(default)]
    pub timeouts: Option<TimeoutOptions>,
    /// `quic_internal` mTLS replication endpoint ([`server.quic_internal`]).
    #[serde(default)]
    pub quic_internal: Option<InternalEndpointOptions>,
    /// `replication` gRPC endpoint ([`server.replication`]).
    #[serde(default)]
    pub replication_endpoint: Option<InternalEndpointOptions>,
    /// Lock-store mode ([`lock_store`]). Defaults to lore's `local`.
    #[serde(default)]
    pub lock_store_mode: Option<String>,
}

/// QUIC transport options. Mirrors lore's `[server.quic]`. Every field is
/// `Option`; omitting a field falls through to lore's compiled-in default
/// (shown in parentheses).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuicOptions {
    /// Override the QUIC port (default: the resolved server port).
    #[serde(default)]
    pub port: Option<u16>,
    /// Require client certificates / mTLS (lore default: `false`).
    #[serde(default)]
    pub verify_client_certs: Option<bool>,
    /// Idle timeout in milliseconds (lore default: `30000`).
    #[serde(default)]
    pub idle_timeout: Option<u64>,
    /// Keep-alive interval in milliseconds (lore default: `500`).
    #[serde(default)]
    pub keep_alive: Option<u64>,
    /// Max concurrent bidirectional streams per connection (lore default: `8`).
    #[serde(default)]
    pub max_bidi_streams: Option<u64>,
    /// Number of QUIC listener tasks (lore default: `10`).
    #[serde(default)]
    pub num_listeners: Option<u8>,
    /// Transport bandwidth cap in bits/second (lore default: `1073741824`, 1 Gbit/s).
    #[serde(default)]
    pub transport_bits_per_second: Option<u64>,
    /// Expected round-trip time in milliseconds (lore default: `100`).
    #[serde(default)]
    pub transport_rtt: Option<u64>,
    /// Per-request handler timeout in seconds (lore default: `50`).
    #[serde(default)]
    pub handler_timeout_seconds: Option<u64>,
    /// Max inflight messages per connection (lore default: unset/unbounded).
    #[serde(default)]
    pub connection_message_limit: Option<u64>,
}

/// gRPC endpoint options. Mirrors lore's `[server.grpc]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrpcOptions {
    /// Override the gRPC port (default: the resolved server port).
    #[serde(default)]
    pub port: Option<u16>,
    /// Require client certificates / mTLS (lore default: `true`).
    #[serde(default)]
    pub verify_client_certs: Option<bool>,
    /// HTTP/2 keepalive ping interval in seconds (lore default: unset).
    #[serde(default)]
    pub http2_keepalive_interval_seconds: Option<u64>,
    /// HTTP/2 keepalive ping timeout in seconds (lore default: unset).
    #[serde(default)]
    pub http2_keepalive_timeout_seconds: Option<u64>,
    /// Per-request handler timeout in seconds (lore default: `50`).
    #[serde(default)]
    pub request_handler_timeout_seconds: Option<u64>,
}

/// HTTP endpoint options. Mirrors lore's `[server.http]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpOptions {
    /// Override the HTTP port (default: server port + 2).
    #[serde(default)]
    pub port: Option<u16>,
    /// Max upload size in bytes (lore default: `10485760`, 10 MB).
    #[serde(default)]
    pub max_file_size: Option<u64>,
    /// Whole-request timeout in seconds (lore default: `300`).
    #[serde(default)]
    pub request_timeout_seconds: Option<u64>,
    /// Request-body read timeout in seconds (lore default: `3600`).
    #[serde(default)]
    pub request_body_timeout_seconds: Option<u64>,
    /// Store-availability poll interval in seconds (lore default: `30`).
    #[serde(default)]
    pub available_interval_seconds: Option<u64>,
    /// Store-availability check timeout in seconds (lore default: `5`).
    #[serde(default)]
    pub available_timeout_seconds: Option<u64>,
    /// Run an active store health check (lore default: `false`).
    #[serde(default)]
    pub store_health_check: Option<bool>,
}

/// Local filesystem store tuning. Applies to whichever stores are local
/// (immutable + mutable in the default case, mutable-only in the S3 case).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalStoreOptions {
    /// Flush interval in seconds (lore default: `10`).
    #[serde(default)]
    pub flush_delay_seconds: Option<u16>,
    /// Immutable-store compaction delay (lore default: unset).
    #[serde(default)]
    pub compaction_delay: Option<u64>,
    /// Immutable-store eviction delay (lore default: unset).
    #[serde(default)]
    pub eviction_delay: Option<u64>,
    /// Immutable-store max capacity in entries (lore default: unset/unbounded).
    #[serde(default)]
    pub max_capacity: Option<u64>,
    /// Immutable-store max on-disk size in bytes (lore default: unset/unbounded).
    #[serde(default)]
    pub max_size: Option<u64>,
}

/// Single-node topology + replication options. The wizard supports lore's
/// built-in `none` / `fixed` / `rotating_id_fixed` providers (no external
/// plugin needed). `consul` and `composite` need plugin or nested config the
/// wizard does not collect and are intentionally not exposed here.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyOptions {
    /// `none` (single node, default), `fixed`, or `rotating_id_fixed`.
    #[serde(default)]
    pub provider: Option<String>,
    /// Peers for `fixed` / `rotating_id_fixed`.
    #[serde(default)]
    pub peers: Vec<PeerOption>,
    /// Rotation interval (seconds) — required when provider is
    /// `rotating_id_fixed`.
    #[serde(default)]
    pub rotation_interval_seconds: Option<u64>,
}

/// A topology peer ([`topology.fixed.peers`] / `rotating_id_fixed.peers`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerOption {
    /// Peer address (host or IP).
    pub address: String,
    /// Peer port.
    pub port: u16,
    /// `SameRegion` or `OtherRegion` (lore's `Locality`).
    #[serde(default)]
    pub locality: Option<String>,
}

/// Telemetry options ([`telemetry`]).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryOptions {
    /// Logger output format: `text` (lore default), `ansi`, or `json`.
    #[serde(default)]
    pub log_format: Option<String>,
    /// Logger output: `stdout` (lore default), `stderr`, or `file`.
    #[serde(default)]
    pub log_output: Option<String>,
    /// File path when `log_output` is `file`.
    #[serde(default)]
    pub log_file: Option<String>,
    /// Emit logs over OTLP (lore default: `false`).
    #[serde(default)]
    pub enable_otlp: Option<bool>,
    /// Metrics export interval in milliseconds (lore default: `30000`).
    #[serde(default)]
    pub metrics_export_interval_millis: Option<u64>,
    /// Metrics sample interval in milliseconds (lore default: `10000`).
    #[serde(default)]
    pub metrics_sample_interval_millis: Option<u64>,
    /// Trace sample rate in `[0.0, 1.0]` (lore default: `0.05`).
    #[serde(default)]
    pub trace_sample_rate: Option<f64>,
    /// Low-tier trace sample rate in `[0.0, 1.0]` (lore default: `0.001`).
    #[serde(default)]
    pub trace_sample_rate_low_tier: Option<f64>,
}

/// Tokio runtime options ([`tokio`]).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOptions {
    /// Worker (async) threads (lore default: unset = number of CPU cores).
    #[serde(default)]
    pub worker_threads: Option<usize>,
    /// Max blocking threads (lore default: `512`).
    #[serde(default)]
    pub max_blocking_threads: Option<usize>,
    /// Idle blocking-thread keep-alive in seconds (lore default varies).
    #[serde(default)]
    pub thread_keep_alive_seconds: Option<u64>,
}

/// Notification backend options ([`notification`]).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationOptions {
    /// Notification mode: `local` (lore default, in-process) or a plugin name.
    #[serde(default)]
    pub mode: Option<String>,
}

/// Revision/history feature-flag options ([`feature`]).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureOptions {
    /// Revision-history step size (lore default: `100`). Must stay small enough
    /// that the cached blob fits the fragment threshold.
    #[serde(default)]
    pub history_step_size: Option<u64>,
    /// Persist `revision_step_key` skip pointers (lore default: `true`).
    #[serde(default)]
    pub revision_step_keys: Option<bool>,
    /// Persist the per-segment revision-list cache (lore default: `true`).
    #[serde(default)]
    pub revision_list_cache: Option<bool>,
    /// Max source-side changes for v1 3-way RevisionDiff (lore default: `100000`).
    #[serde(default)]
    pub revision_diff_source_cap: Option<u64>,
    /// Parallel history-walk permits for diff3 (lore default: `24`).
    #[serde(default)]
    pub revision_diff_history_walk_concurrency: Option<u64>,
}

/// Graceful-shutdown timeout options ([`server`]).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeoutOptions {
    /// Seconds to wait for connections to drain on shutdown (lore default: `5`).
    #[serde(default)]
    pub connection_close_timeout_seconds: Option<u16>,
    /// Seconds to wait for the runtime to stop after draining (lore default: `25`).
    #[serde(default)]
    pub runtime_shutdown_timeout_seconds: Option<u16>,
}

/// Options for the internal `quic_internal` / `replication` endpoints. These
/// are opt-in (`enabled = false` by lore default) and require mTLS certs.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InternalEndpointOptions {
    /// Enable the endpoint (lore default: `false`).
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Bind port (lore default: `41340`).
    #[serde(default)]
    pub port: Option<u16>,
    /// mTLS certificate chain file.
    #[serde(default)]
    pub cert_chain: Option<String>,
    /// mTLS certificate file (required when `enabled`).
    #[serde(default)]
    pub cert_file: Option<String>,
    /// mTLS private-key file (required when `enabled`).
    #[serde(default)]
    pub pkey_file: Option<String>,
}

/// S3-compatible object-storage options for the hosted server's immutable store.
///
/// Mirrors lore's `[plugins.aws.immutable_store]` S3 keys (`s3_bucket`,
/// `s3_endpoint_url`, `s3_region`, `s3_force_path_style`). The same shape works
/// for AWS S3, MinIO, Garage, Ceph/RGW, Backblaze B2, etc. — they differ only by
/// endpoint URL and whether path-style addressing is required.
///
/// Credentials are NOT written into the TOML: lore's AWS plugin resolves them
/// through the standard AWS credential chain, so [`access_key_id`] /
/// [`secret_access_key`] are exported to the spawned `loreserver` process as the
/// `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` environment variables instead.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct S3StoreOptions {
    /// S3 endpoint URL. Empty/`None` selects real AWS S3 (SDK default endpoint).
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Bucket name. Required — an `aws`-mode immutable store needs a bucket.
    pub bucket: String,
    /// Region (e.g. `"us-east-1"`). Required by most S3 providers.
    #[serde(default)]
    pub region: Option<String>,
    /// Access key id, exported as `AWS_ACCESS_KEY_ID` to the server process.
    #[serde(default)]
    pub access_key_id: Option<String>,
    /// Secret access key, exported as `AWS_SECRET_ACCESS_KEY`.
    #[serde(default)]
    pub secret_access_key: Option<String>,
    /// Force path-style addressing (`endpoint/bucket/key`). MinIO/Garage and most
    /// non-AWS providers require this; real AWS S3 does not. Defaults to `false`.
    #[serde(default)]
    pub force_path_style: bool,
    /// Optional DynamoDB-compatible endpoint URL for the immutable store's
    /// fragment-association + metadata tables.
    ///
    /// lore's `aws` immutable store pairs S3 (fragment payloads) with DynamoDB
    /// (fragment associations + metadata) — there is no S3-only immutable store.
    /// When omitted, the AWS SDK resolves the real AWS DynamoDB service in the
    /// chosen region (so S3-on-AWS + DynamoDB-on-AWS works out of the box).
    /// Operators running a DynamoDB-compatible service (DynamoDB Local,
    /// LocalStack, ScyllaDB Alternator, …) set this to that endpoint.
    #[serde(default)]
    pub dynamodb_endpoint: Option<String>,
}

/// A running hosted server plus the metadata the UI needs.
pub struct HostedServer {
    /// The managed child process. `None` only transiently during teardown.
    child: Option<Child>,
    /// OS process id of the server.
    pub pid: u32,
    /// QUIC/gRPC port.
    pub port: u16,
    /// HTTP port (`port + 2`).
    pub http_port: u16,
    /// Advertised `lore://host:port[/<repo>]` URL clients connect to.
    pub url: String,
    /// Path to the generated config file on disk.
    pub config_path: PathBuf,
    /// Store directory being served.
    pub store_dir: PathBuf,
}

/// Serializable status returned to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub http_port: Option<u16>,
    pub url: Option<String>,
    pub config_path: Option<String>,
    pub store_dir: Option<String>,
}

impl HostStatus {
    fn stopped() -> Self {
        HostStatus {
            running: false,
            pid: None,
            port: None,
            http_port: None,
            url: None,
            config_path: None,
            store_dir: None,
        }
    }

    fn from(server: &HostedServer) -> Self {
        HostStatus {
            running: true,
            pid: Some(server.pid),
            port: Some(server.port),
            http_port: Some(server.http_port),
            url: Some(server.url.clone()),
            config_path: Some(server.config_path.to_string_lossy().into_owned()),
            store_dir: Some(server.store_dir.to_string_lossy().into_owned()),
        }
    }
}

/// Resolved, fully-qualified inputs for config generation.
struct ResolvedConfig {
    /// Bind host for every endpoint. Defaults to [`BIND_HOST`].
    bind_host: String,
    port: u16,
    http_port: u16,
    store_dir: PathBuf,
    cert_file: PathBuf,
    pkey_file: PathBuf,
    auth: bool,
    /// When `Some`, the immutable store is emitted in lore's `aws` (S3) mode
    /// instead of `local`. The mutable store is always local (see
    /// [`S3StoreOptions`]).
    s3: Option<ResolvedS3>,
    /// Validated advanced sections (SBAI-4075). Each is rendered only when the
    /// user supplied an explicit, non-default value. The whole bag defaults to
    /// all-`None`, reproducing the original minimal local config exactly.
    adv: AdvancedConfig,
}

/// Validated advanced-config sections carried through to the renderer.
/// All-`Default` (every field `None`/empty) renders nothing extra.
#[derive(Debug, Clone, Default)]
struct AdvancedConfig {
    quic: Option<QuicOptions>,
    grpc: Option<GrpcOptions>,
    http: Option<HttpOptions>,
    local_store: Option<LocalStoreOptions>,
    topology: Option<TopologyOptions>,
    telemetry: Option<TelemetryOptions>,
    runtime: Option<RuntimeOptions>,
    notification: Option<NotificationOptions>,
    features: Option<FeatureOptions>,
    timeouts: Option<TimeoutOptions>,
    quic_internal: Option<InternalEndpointOptions>,
    replication_endpoint: Option<InternalEndpointOptions>,
    lock_store_mode: Option<String>,
}

/// Resolved S3 inputs for `[plugins.aws.immutable_store]` generation.
#[derive(Debug, Clone)]
struct ResolvedS3 {
    endpoint: Option<String>,
    bucket: String,
    region: Option<String>,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    force_path_style: bool,
    dynamodb_endpoint: Option<String>,
}

impl ResolvedS3 {
    /// DynamoDB table names for the immutable store, derived from the bucket so a
    /// single S3 backend is self-describing. lore auto-ensures these tables.
    fn fragments_table(&self) -> String {
        format!("{}-fragments", self.bucket)
    }
    fn metadata_table(&self) -> String {
        format!("{}-fragment-metadata", self.bucket)
    }
}

/// Render the `loreserver` config TOML from resolved inputs.
///
/// Pure and deterministic so it can be unit-tested. Mirrors the spike's
/// `local.toml`: localhost QUIC + gRPC on the same port number (TCP gRPC / UDP
/// QUIC), HTTP on `port + 2`, the shipped test certs for QUIC, single-node
/// topology, and — crucially — **no `[server.auth]` block** so the server runs
/// auth-disabled.
///
/// The **immutable** store is one of lore's two real backends:
///   - `local` (default): a filesystem store under the chosen directory.
///   - `aws` (when [`ResolvedConfig::s3`] is set): an S3-compatible object store
///     (AWS S3 / MinIO / Garage / Ceph-RGW / B2 / …, all the same backend) for
///     fragment payloads, paired with DynamoDB for fragment associations +
///     metadata (lore's `aws` immutable store has no S3-only variant).
///
/// The **mutable** (branch-pointer) store is always `local`: lore's `aws`
/// mutable store needs a dedicated DynamoDB table the host wizard does not
/// provision, and an S3-immutable + local-mutable single node is a valid lore
/// topology (cf. upstream `composite.local + aws.durable`).
fn render_config_toml(cfg: &ResolvedConfig) -> String {
    // Paths are emitted as TOML basic strings; escape backslashes (Windows) and
    // quotes so the file is valid regardless of the platform's path separators.
    let esc = |p: &Path| -> String {
        p.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    };
    // Escape a plain string value for a TOML basic string.
    let escs = |s: &str| -> String { s.replace('\\', "\\\\").replace('"', "\\\"") };

    // Escape a quoted host value (also used for the bind host).
    let host = escs(&cfg.bind_host);
    let adv = &cfg.adv;

    let mut out = String::new();
    out.push_str(
        "# Generated by LoreGUI \"Host a server\" (SBAI-4065/SBAI-4075). Do not edit by hand.\n",
    );
    out.push_str("# Only non-default values are emitted; every omitted key falls back to lore's\n");
    out.push_str("# own compiled-in default (config/default.toml).\n");
    if cfg.s3.is_some() {
        out.push_str("# loreserver: S3-compatible immutable store + local mutable store.\n\n");
    } else {
        out.push_str("# Single-node, loopback-only, local-store loreserver config.\n\n");
    }

    // -- QUIC (public endpoint) --
    out.push_str("[server.quic]\n");
    out.push_str(&format!("host = \"{host}\"\n"));
    out.push_str(&format!(
        "port = {}\n",
        adv.quic.as_ref().and_then(|q| q.port).unwrap_or(cfg.port)
    ));
    if let Some(q) = &adv.quic {
        if let Some(v) = q.verify_client_certs {
            out.push_str(&format!("verify_client_certs = {v}\n"));
        }
        if let Some(v) = q.idle_timeout {
            out.push_str(&format!("idle_timeout = {v}\n"));
        }
        if let Some(v) = q.keep_alive {
            out.push_str(&format!("keep_alive = {v}\n"));
        }
        if let Some(v) = q.max_bidi_streams {
            out.push_str(&format!("max_bidi_streams = {v}\n"));
        }
        if let Some(v) = q.num_listeners {
            out.push_str(&format!("num_listeners = {v}\n"));
        }
        if let Some(v) = q.transport_bits_per_second {
            out.push_str(&format!("transport_bits_per_second = {v}\n"));
        }
        if let Some(v) = q.transport_rtt {
            out.push_str(&format!("transport_rtt = {v}\n"));
        }
        if let Some(v) = q.handler_timeout_seconds {
            out.push_str(&format!("handler_timeout_seconds = {v}\n"));
        }
        if let Some(v) = q.connection_message_limit {
            out.push_str(&format!("connection_message_limit = {v}\n"));
        }
    }
    out.push_str("[server.quic.certificate]\n");
    out.push_str(&format!("cert_file = \"{}\"\n", esc(&cfg.cert_file)));
    out.push_str(&format!("pkey_file = \"{}\"\n\n", esc(&cfg.pkey_file)));

    // -- gRPC --
    out.push_str("[server.grpc]\n");
    out.push_str(&format!("host = \"{host}\"\n"));
    out.push_str(&format!(
        "port = {}\n",
        adv.grpc.as_ref().and_then(|g| g.port).unwrap_or(cfg.port)
    ));
    if let Some(g) = &adv.grpc {
        if let Some(v) = g.verify_client_certs {
            out.push_str(&format!("verify_client_certs = {v}\n"));
        }
        if let Some(v) = g.http2_keepalive_interval_seconds {
            out.push_str(&format!("http2_keepalive_interval_seconds = {v}\n"));
        }
        if let Some(v) = g.http2_keepalive_timeout_seconds {
            out.push_str(&format!("http2_keepalive_timeout_seconds = {v}\n"));
        }
        if let Some(v) = g.request_handler_timeout_seconds {
            out.push_str(&format!("request_handler_timeout_seconds = {v}\n"));
        }
    }
    out.push('\n');

    // -- HTTP --
    out.push_str("[server.http]\n");
    out.push_str(&format!("host = \"{host}\"\n"));
    out.push_str(&format!(
        "port = {}\n",
        adv.http
            .as_ref()
            .and_then(|h| h.port)
            .unwrap_or(cfg.http_port)
    ));
    if let Some(h) = &adv.http {
        if let Some(v) = h.max_file_size {
            out.push_str(&format!("max_file_size = {v}\n"));
        }
        if let Some(v) = h.request_timeout_seconds {
            out.push_str(&format!("request_timeout_seconds = {v}\n"));
        }
        if let Some(v) = h.request_body_timeout_seconds {
            out.push_str(&format!("request_body_timeout_seconds = {v}\n"));
        }
        if let Some(v) = h.available_interval_seconds {
            out.push_str(&format!("available_interval_seconds = {v}\n"));
        }
        if let Some(v) = h.available_timeout_seconds {
            out.push_str(&format!("available_timeout_seconds = {v}\n"));
        }
        if let Some(v) = h.store_health_check {
            out.push_str(&format!("store_health_check = {v}\n"));
        }
    }
    out.push('\n');

    // -- Internal endpoints (quic_internal / replication), opt-in + mTLS --
    render_internal_endpoint(&mut out, "server.quic_internal", &adv.quic_internal, &escs);
    render_internal_endpoint(
        &mut out,
        "server.replication",
        &adv.replication_endpoint,
        &escs,
    );

    // -- Graceful-shutdown timeouts --
    if let Some(t) = &adv.timeouts {
        let mut wrote = false;
        if let Some(v) = t.connection_close_timeout_seconds {
            if !wrote {
                out.push_str("[server]\n");
                wrote = true;
            }
            out.push_str(&format!("connection_close_timeout_seconds = {v}\n"));
        }
        if let Some(v) = t.runtime_shutdown_timeout_seconds {
            if !wrote {
                out.push_str("[server]\n");
                wrote = true;
            }
            out.push_str(&format!("runtime_shutdown_timeout_seconds = {v}\n"));
        }
        if wrote {
            out.push('\n');
        }
    }

    // -- Stores --
    let local_path = esc(&cfg.store_dir);
    let render_local_extras =
        |out: &mut String, ls: &Option<LocalStoreOptions>, immutable: bool| {
            if let Some(ls) = ls {
                if let Some(v) = ls.flush_delay_seconds {
                    out.push_str(&format!("flush_delay_seconds = {v}\n"));
                }
                if immutable {
                    if let Some(v) = ls.compaction_delay {
                        out.push_str(&format!("compaction_delay = {v}\n"));
                    }
                    if let Some(v) = ls.eviction_delay {
                        out.push_str(&format!("eviction_delay = {v}\n"));
                    }
                    if let Some(v) = ls.max_capacity {
                        out.push_str(&format!("max_capacity = {v}\n"));
                    }
                    if let Some(v) = ls.max_size {
                        out.push_str(&format!("max_size = {v}\n"));
                    }
                }
            }
        };

    match &cfg.s3 {
        // Local filesystem immutable + mutable stores (the default).
        None => {
            out.push_str("[immutable_store.local]\n");
            out.push_str(&format!("path = \"{local_path}\"\n"));
            render_local_extras(&mut out, &adv.local_store, true);
            out.push_str("[mutable_store.local]\n");
            out.push_str(&format!("path = \"{local_path}\"\n"));
            render_local_extras(&mut out, &adv.local_store, false);
            out.push('\n');
        }
        // S3-compatible (lore `aws` mode) immutable store; local mutable store.
        Some(s3) => {
            out.push_str("[immutable_store]\n");
            out.push_str("mode = \"aws\"\n\n");

            // Mutable (branch pointers) stays on local disk — see fn docs.
            out.push_str("[mutable_store.local]\n");
            out.push_str(&format!("path = \"{local_path}\"\n"));
            render_local_extras(&mut out, &adv.local_store, false);
            out.push('\n');

            // The aws immutable store plugin: S3 for payloads, DynamoDB for the
            // fragment-association + metadata tables (auto-ensured at startup).
            out.push_str("[plugins.aws.immutable_store]\n");
            out.push_str(&format!("s3_bucket = \"{}\"\n", escs(&s3.bucket)));
            if let Some(ep) = &s3.endpoint {
                out.push_str(&format!("s3_endpoint_url = \"{}\"\n", escs(ep)));
            }
            if let Some(region) = &s3.region {
                out.push_str(&format!("s3_region = \"{}\"\n", escs(region)));
            }
            out.push_str(&format!("s3_force_path_style = {}\n", s3.force_path_style));
            out.push_str(&format!(
                "dynamodb_fragments_table = \"{}\"\n",
                escs(&s3.fragments_table())
            ));
            out.push_str(&format!(
                "dynamodb_metadata_table = \"{}\"\n",
                escs(&s3.metadata_table())
            ));
            if let Some(ddb) = &s3.dynamodb_endpoint {
                out.push_str(&format!("dynamodb_endpoint_url = \"{}\"\n", escs(ddb)));
            }
            // DynamoDB shares the S3 region unless the user runs it elsewhere.
            if let Some(region) = &s3.region {
                out.push_str(&format!("dynamodb_region = \"{}\"\n", escs(region)));
            }
            out.push('\n');
        }
    }

    // -- Lock store mode (lore default: local) --
    if let Some(mode) = &adv.lock_store_mode {
        out.push_str("[lock_store]\n");
        out.push_str(&format!("mode = \"{}\"\n\n", escs(mode)));
    }

    // -- Telemetry --
    // The default (no override) keeps the original `format = "ansi"` line so the
    // simple first-run config is byte-for-byte what it always was.
    match &adv.telemetry {
        None => {
            out.push_str("[telemetry.logger]\n");
            out.push_str("format = \"ansi\"\n\n");
        }
        Some(t) => {
            out.push_str("[telemetry.logger]\n");
            out.push_str(&format!(
                "format = \"{}\"\n",
                t.log_format.as_deref().unwrap_or("ansi")
            ));
            match t.log_output.as_deref() {
                Some("file") => {
                    let path = t.log_file.as_deref().unwrap_or("lore-server.log");
                    out.push_str(&format!("output = {{ file = \"{}\" }}\n", escs(path)));
                }
                Some(other @ ("stdout" | "stderr")) => {
                    out.push_str(&format!("output = \"{other}\"\n"));
                }
                _ => {}
            }
            if let Some(v) = t.enable_otlp {
                out.push_str(&format!("enable_otlp = {v}\n"));
            }
            out.push('\n');
            if t.metrics_export_interval_millis.is_some()
                || t.metrics_sample_interval_millis.is_some()
            {
                out.push_str("[telemetry.metrics]\n");
                if let Some(v) = t.metrics_export_interval_millis {
                    out.push_str(&format!("export_interval_millis = {v}\n"));
                }
                if let Some(v) = t.metrics_sample_interval_millis {
                    out.push_str(&format!("sample_interval_millis = {v}\n"));
                }
                out.push('\n');
            }
            if t.trace_sample_rate.is_some() || t.trace_sample_rate_low_tier.is_some() {
                out.push_str("[telemetry.traces]\n");
                if let Some(v) = t.trace_sample_rate {
                    out.push_str(&format!("sample_rate = {v}\n"));
                }
                if let Some(v) = t.trace_sample_rate_low_tier {
                    out.push_str(&format!("sample_rate_low_tier = {v}\n"));
                }
                out.push('\n');
            }
        }
    }

    // -- Tokio runtime --
    if let Some(r) = &adv.runtime {
        if r.worker_threads.is_some()
            || r.max_blocking_threads.is_some()
            || r.thread_keep_alive_seconds.is_some()
        {
            out.push_str("[tokio]\n");
            if let Some(v) = r.worker_threads {
                out.push_str(&format!("worker_threads = {v}\n"));
            }
            if let Some(v) = r.max_blocking_threads {
                out.push_str(&format!("max_blocking_threads = {v}\n"));
            }
            if let Some(v) = r.thread_keep_alive_seconds {
                out.push_str(&format!("thread_keep_alive_seconds = {v}\n"));
            }
            out.push('\n');
        }
    }

    // -- Notification --
    if let Some(n) = &adv.notification {
        if let Some(mode) = &n.mode {
            out.push_str("[notification]\n");
            out.push_str(&format!("mode = \"{}\"\n\n", escs(mode)));
        }
    }

    // -- Feature flags --
    if let Some(f) = &adv.features {
        let any = f.history_step_size.is_some()
            || f.revision_step_keys.is_some()
            || f.revision_list_cache.is_some()
            || f.revision_diff_source_cap.is_some()
            || f.revision_diff_history_walk_concurrency.is_some();
        if any {
            out.push_str("[feature]\n");
            if let Some(v) = f.history_step_size {
                out.push_str(&format!("history_step_size = {v}\n"));
            }
            if let Some(v) = f.revision_step_keys {
                out.push_str(&format!("revision_step_keys = {v}\n"));
            }
            if let Some(v) = f.revision_list_cache {
                out.push_str(&format!("revision_list_cache = {v}\n"));
            }
            if let Some(v) = f.revision_diff_source_cap {
                out.push_str(&format!("revision_diff_source_cap = {v}\n"));
            }
            if let Some(v) = f.revision_diff_history_walk_concurrency {
                out.push_str(&format!("revision_diff_history_walk_concurrency = {v}\n"));
            }
            out.push('\n');
        }
    }

    // -- Topology --
    // Default (no override) is single-node `provider = "none"`, preserved exactly.
    render_topology(&mut out, &adv.topology, &escs);

    // Auth hook: a future authed mode would append a `[server.auth]` block here.
    // The no-auth local host flow deliberately omits it (server logs
    // "Auth: disabled"). Keep the branch explicit so the intent is documented.
    if cfg.auth {
        out.push_str("\n# NOTE: authed hosting is not yet implemented; running auth-disabled.\n");
    }

    // FOLLOW-UP (advanced / enterprise lore store modes, deferred — see
    // docs/domains/storage.md): lore also supports a `composite` immutable store
    // (local cache tier + durable `aws`/S3 tier with a `ReplicationMode` of
    // read/write/read_write), a full `replicated` store (server-to-server QUIC
    // with replica peers + a replica factory), an `aws` (DynamoDB) mutable +
    // lock store at scale, and `consul`/`composite` topology providers. They
    // need plugin config / nested inputs the host wizard does not collect, so
    // they are intentionally not emitted here. Add new `*Options` + render
    // branches to wire them when those flows land.

    out
}

/// Render an opt-in internal endpoint (`quic_internal` / `replication`). These
/// default to `enabled = false`, so nothing is emitted unless the user supplied
/// at least one explicit value. mTLS certs are nested under `<section>.certificate`.
fn render_internal_endpoint(
    out: &mut String,
    section: &str,
    opts: &Option<InternalEndpointOptions>,
    escs: &impl Fn(&str) -> String,
) {
    let Some(o) = opts else { return };
    let any = o.enabled.is_some()
        || o.port.is_some()
        || o.cert_chain.is_some()
        || o.cert_file.is_some()
        || o.pkey_file.is_some();
    if !any {
        return;
    }
    out.push_str(&format!("[{section}]\n"));
    if let Some(v) = o.enabled {
        out.push_str(&format!("enabled = {v}\n"));
    }
    if let Some(v) = o.port {
        out.push_str(&format!("port = {v}\n"));
    }
    if o.cert_chain.is_some() || o.cert_file.is_some() || o.pkey_file.is_some() {
        out.push_str(&format!("[{section}.certificate]\n"));
        if let Some(v) = &o.cert_chain {
            out.push_str(&format!("cert_chain = \"{}\"\n", escs(v)));
        }
        if let Some(v) = &o.cert_file {
            out.push_str(&format!("cert_file = \"{}\"\n", escs(v)));
        }
        if let Some(v) = &o.pkey_file {
            out.push_str(&format!("pkey_file = \"{}\"\n", escs(v)));
        }
    }
    out.push('\n');
}

/// Render the `[topology]` section. Defaults to single-node `provider = "none"`.
fn render_topology(
    out: &mut String,
    opts: &Option<TopologyOptions>,
    escs: &impl Fn(&str) -> String,
) {
    let provider = opts
        .as_ref()
        .and_then(|t| t.provider.as_deref())
        .filter(|p| !p.trim().is_empty())
        .unwrap_or("none");
    out.push_str("[topology]\n");
    out.push_str(&format!("provider = \"{provider}\"\n"));

    let Some(t) = opts else { return };
    if provider == "none" {
        return;
    }
    // rotating_id_fixed needs a rotation interval alongside the peer list.
    let section = if provider == "rotating_id_fixed" {
        "topology.rotating_id_fixed"
    } else {
        "topology.fixed"
    };
    if provider == "rotating_id_fixed" {
        if let Some(v) = t.rotation_interval_seconds {
            out.push_str(&format!("\n[{section}]\n"));
            out.push_str(&format!("rotation_interval_seconds = {v}\n"));
        } else if !t.peers.is_empty() {
            out.push_str(&format!("\n[{section}]\n"));
        }
    }
    for p in &t.peers {
        let locality = p.locality.as_deref().unwrap_or("SameRegion");
        out.push_str(&format!(
            "[[{section}.peers]]\naddress = \"{}\"\nport = {}\nlocality = \"{}\"\n",
            escs(&p.address),
            p.port,
            escs(locality),
        ));
    }
}

/// The advertised connection URL. `lore://` (no trailing `s`) so clients skip
/// server-cert validation against the self-signed test cert (see spike).
fn advertise_url(port: u16, repository_name: Option<&str>) -> String {
    match repository_name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => format!("lore://{BIND_HOST}:{port}/{name}"),
        None => format!("lore://{BIND_HOST}:{port}"),
    }
}

/// Locate the upstream `lore` git checkout cargo unpacked for the pinned rev.
///
/// `Cargo.toml` pins `lore` by 40-char rev; cargo unpacks it under
/// `$CARGO_HOME/git/checkouts/lore-*/<short-rev>/`. We read the rev from the
/// workspace `Cargo.toml` and find the matching short-rev dir — exactly as the
/// spike script does.
fn lore_checkout() -> Result<PathBuf, LoreError> {
    // src-tauri/Cargo.toml is one level above this crate's manifest dir; the
    // pinned rev lives in the *workspace* Cargo.toml at the repo root.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent().ok_or_else(|| {
        LoreError::CommandFailed("could not locate repo root from CARGO_MANIFEST_DIR".into())
    })?;
    let cargo_toml = repo_root.join("Cargo.toml");
    let text = std::fs::read_to_string(&cargo_toml).map_err(|e| {
        LoreError::CommandFailed(format!(
            "could not read {} to find pinned lore rev: {e}",
            cargo_toml.display()
        ))
    })?;
    let rev = parse_pinned_rev(&text).ok_or_else(|| {
        LoreError::CommandFailed("could not parse pinned lore rev from Cargo.toml".into())
    })?;
    let short = &rev[..7];

    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs_home().map(|h| h.join(".cargo")))
        .ok_or_else(|| LoreError::CommandFailed("could not resolve CARGO_HOME".into()))?;
    let checkouts = cargo_home.join("git").join("checkouts");

    // checkouts/lore-<hash>/<short-rev>/
    let entries = std::fs::read_dir(&checkouts).map_err(|e| {
        LoreError::CommandFailed(format!(
            "lore git checkout not found under {}: {e} — run a build that fetches the dep first",
            checkouts.display()
        ))
    })?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("lore-") {
            let candidate = entry.path().join(short);
            if candidate.is_dir() {
                return Ok(candidate);
            }
        }
    }
    Err(LoreError::CommandFailed(format!(
        "lore checkout for rev {short} not found under {} — run `cargo fetch` first",
        checkouts.display()
    )))
}

/// Extract the first 40-hex-char `rev = "..."` from a Cargo.toml string.
fn parse_pinned_rev(cargo_toml: &str) -> Option<String> {
    for line in cargo_toml.lines() {
        if let Some(idx) = line.find("rev = \"") {
            let rest = &line[idx + "rev = \"".len()..];
            if let Some(end) = rest.find('"') {
                let rev = &rest[..end];
                if rev.len() == 40 && rev.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Some(rev.to_string());
                }
            }
        }
    }
    None
}

/// Best-effort home directory (avoids pulling in the `dirs` crate).
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Resolve the `loreserver` binary, building it from the dev checkout if needed.
///
/// Order:
///   1. `LOREVM_SERVER_BIN` env var (explicit override).
///   2. A Tauri **sidecar** next to the current executable (`loreserver`
///      / `loreserver.exe`) — present only once we bundle it as `externalBin`.
///   3. DEV fallback: the pinned upstream checkout's
///      `target/debug/loreserver`, built via `cargo build -p lore-server
///      --bin loreserver` if absent (exactly as the spike script does).
///
/// FOLLOW-UP: production should ship `loreserver` as a Tauri sidecar
/// (`externalBin` in `tauri.conf.json`) so step 3 is never reached in a
/// release build. We intentionally do NOT add the ~1 GB debug binary to the
/// bundle / CI now — it is resolved at runtime here instead.
fn resolve_server_binary() -> Result<PathBuf, LoreError> {
    // 1. explicit override
    if let Some(p) = std::env::var_os("LOREVM_SERVER_BIN") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Ok(path);
        }
        return Err(LoreError::CommandFailed(format!(
            "LOREVM_SERVER_BIN={} is not a file",
            path.display()
        )));
    }

    // 2. sidecar next to the running executable
    if let Some(p) = sidecar_candidate() {
        if p.is_file() {
            return Ok(p);
        }
    }

    // 3. dev fallback: build from the pinned upstream checkout
    let checkout = lore_checkout()?;
    let bin_name = if cfg!(windows) {
        "loreserver.exe"
    } else {
        "loreserver"
    };
    let built = checkout.join("target").join("debug").join(bin_name);
    if built.is_file() {
        return Ok(built);
    }

    // Build it (first run is slow — several minutes, ~1 GB debug binary).
    tracing::info!(
        "loreserver not built; running `cargo build -p lore-server --bin loreserver` in {}",
        checkout.display()
    );
    let status = Command::new("cargo")
        .args(["build", "-p", "lore-server", "--bin", "loreserver"])
        .current_dir(&checkout)
        .status()
        .map_err(|e| {
            LoreError::CommandFailed(format!("failed to launch cargo to build loreserver: {e}"))
        })?;
    if !status.success() {
        return Err(LoreError::CommandFailed(
            "cargo build -p lore-server --bin loreserver failed".into(),
        ));
    }
    if built.is_file() {
        Ok(built)
    } else {
        Err(LoreError::CommandFailed(format!(
            "built loreserver not found at {}",
            built.display()
        )))
    }
}

/// Candidate sidecar path next to the current executable.
fn sidecar_candidate() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let bin_name = if cfg!(windows) {
        "loreserver.exe"
    } else {
        "loreserver"
    };
    Some(dir.join(bin_name))
}

/// Validate + normalise the wizard's S3 options into [`ResolvedS3`].
///
/// Trims string fields and treats blanks as absent. The bucket is required —
/// an `aws`-mode immutable store cannot be created without one.
fn resolve_s3(opts: &S3StoreOptions) -> Result<ResolvedS3, LoreError> {
    let bucket = opts.bucket.trim();
    if bucket.is_empty() {
        return Err(LoreError::CommandFailed(
            "an S3-compatible bucket name is required to host with object storage".into(),
        ));
    }

    Ok(ResolvedS3 {
        endpoint: norm_str(&opts.endpoint),
        bucket: bucket.to_owned(),
        region: norm_str(&opts.region),
        access_key_id: norm_str(&opts.access_key_id),
        secret_access_key: norm_str(&opts.secret_access_key),
        force_path_style: opts.force_path_style,
        dynamodb_endpoint: norm_str(&opts.dynamodb_endpoint),
    })
}

/// Trim a `String`; treat blank as `None`.
fn norm_str(s: &Option<String>) -> Option<String> {
    s.as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

/// Validate + normalise the advanced sections (SBAI-4075). Returns an
/// [`AdvancedConfig`] carrying only the user-supplied, sane values; everything
/// else stays `None` so lore's own defaults apply. Surfaces actionable errors
/// for out-of-range / required-when-mode mistakes the wizard's client-side
/// validation should already prevent (defence in depth).
fn resolve_advanced(opts: &HostServerOptions) -> Result<AdvancedConfig, LoreError> {
    let err = |m: String| LoreError::CommandFailed(m);

    // No Expert-mode bag → nothing extra; lore defaults apply everywhere.
    let Some(adv) = opts.advanced.as_ref() else {
        return Ok(AdvancedConfig::default());
    };

    // Telemetry: enum + range checks.
    if let Some(t) = &adv.telemetry {
        if let Some(f) = &t.log_format {
            if !matches!(f.as_str(), "text" | "ansi" | "json") {
                return Err(err(format!(
                    "telemetry log format must be text, ansi, or json (got {f:?})"
                )));
            }
        }
        if let Some(o) = &t.log_output {
            if !matches!(o.as_str(), "stdout" | "stderr" | "file") {
                return Err(err(format!(
                    "telemetry log output must be stdout, stderr, or file (got {o:?})"
                )));
            }
            if o == "file" && norm_str(&t.log_file).is_none() {
                return Err(err(
                    "a log file path is required when telemetry output is 'file'".into(),
                ));
            }
        }
        for (name, v) in [
            ("trace sample rate", t.trace_sample_rate),
            ("low-tier trace sample rate", t.trace_sample_rate_low_tier),
        ] {
            if let Some(v) = v {
                if !(0.0..=1.0).contains(&v) {
                    return Err(err(format!("{name} must be within [0.0, 1.0] (got {v})")));
                }
            }
        }
    }

    // Topology: validate provider + required-when-mode + peers.
    let topology = if let Some(t) = &adv.topology {
        let provider = norm_str(&t.provider).unwrap_or_else(|| "none".into());
        if !matches!(provider.as_str(), "none" | "fixed" | "rotating_id_fixed") {
            return Err(err(format!(
                "topology provider must be none, fixed, or rotating_id_fixed (got {provider:?})"
            )));
        }
        let mut peers = Vec::new();
        for p in &t.peers {
            let address = p.address.trim();
            if address.is_empty() {
                return Err(err("every topology peer needs an address".into()));
            }
            if let Some(loc) = &p.locality {
                if !matches!(loc.as_str(), "SameRegion" | "OtherRegion") {
                    return Err(err(format!(
                        "peer locality must be SameRegion or OtherRegion (got {loc:?})"
                    )));
                }
            }
            peers.push(PeerOption {
                address: address.to_owned(),
                port: p.port,
                locality: norm_str(&p.locality),
            });
        }
        if provider != "none" && peers.is_empty() {
            return Err(err(format!(
                "topology provider {provider:?} requires at least one peer"
            )));
        }
        if provider == "rotating_id_fixed" && t.rotation_interval_seconds.is_none() {
            return Err(err(
                "rotating_id_fixed topology requires a rotation interval (seconds)".into(),
            ));
        }
        Some(TopologyOptions {
            provider: Some(provider),
            peers,
            rotation_interval_seconds: t.rotation_interval_seconds,
        })
    } else {
        None
    };

    // Internal endpoints: when enabled, require the mTLS cert + key.
    let resolve_internal = |o: &Option<InternalEndpointOptions>,
                            name: &str|
     -> Result<Option<InternalEndpointOptions>, LoreError> {
        let Some(o) = o else { return Ok(None) };
        let cert_file = norm_str(&o.cert_file);
        let pkey_file = norm_str(&o.pkey_file);
        if o.enabled == Some(true) && (cert_file.is_none() || pkey_file.is_none()) {
            return Err(err(format!(
                "the {name} endpoint requires an mTLS cert file and key when enabled"
            )));
        }
        Ok(Some(InternalEndpointOptions {
            enabled: o.enabled,
            port: o.port,
            cert_chain: norm_str(&o.cert_chain),
            cert_file,
            pkey_file,
        }))
    };

    Ok(AdvancedConfig {
        quic: adv.quic.clone(),
        grpc: adv.grpc.clone(),
        http: adv.http.clone(),
        local_store: adv.local_store.clone(),
        topology,
        telemetry: adv.telemetry.as_ref().map(|t| TelemetryOptions {
            log_format: norm_str(&t.log_format),
            log_output: norm_str(&t.log_output),
            log_file: norm_str(&t.log_file),
            ..t.clone()
        }),
        runtime: adv.runtime.clone(),
        notification: adv.notification.as_ref().map(|n| NotificationOptions {
            mode: norm_str(&n.mode),
        }),
        features: adv.features.clone(),
        timeouts: adv.timeouts.clone(),
        quic_internal: resolve_internal(&adv.quic_internal, "quic_internal")?,
        replication_endpoint: resolve_internal(&adv.replication_endpoint, "replication")?,
        lock_store_mode: norm_str(&adv.lock_store_mode),
    })
}

/// Resolve every input into a [`ResolvedConfig`] without touching the
/// filesystem store dir or writing the config file. Used by both [`prepare`]
/// (which then writes the file) and the "view config" preview command, so a
/// preview never spawns a server or mutates disk.
///
/// `cert_file` / `pkey_file` are the shipped self-signed QUIC test certs from
/// the pinned lore checkout. When the checkout isn't present yet (no build has
/// fetched the dep), `allow_missing_certs` lets the preview fall back to clearly
/// labelled placeholder paths instead of erroring.
fn resolve_config(
    opts: &HostServerOptions,
    allow_missing_certs: bool,
) -> Result<ResolvedConfig, LoreError> {
    if opts.store_dir.trim().is_empty() {
        return Err(LoreError::CommandFailed(
            "store directory is required to host a server".into(),
        ));
    }
    let store_dir = PathBuf::from(opts.store_dir.trim());

    let port = match opts.port {
        Some(p) if p != 0 => p,
        _ => DEFAULT_PORT,
    };
    let http_port = port.wrapping_add(2);

    let (cert_file, pkey_file) = match lore_checkout() {
        Ok(checkout) => {
            let test_data = checkout
                .join("lore-server")
                .join("src")
                .join("protocol")
                .join("test_data");
            (
                test_data.join("test_cert.pem"),
                test_data.join("test_key.pem"),
            )
        }
        Err(e) if allow_missing_certs => {
            tracing::debug!("lore checkout unavailable for cert paths in preview: {e}");
            (
                PathBuf::from("<lore test_cert.pem>"),
                PathBuf::from("<lore test_key.pem>"),
            )
        }
        Err(e) => return Err(e),
    };

    let bind_host = norm_str(&opts.bind_host).unwrap_or_else(|| BIND_HOST.to_owned());
    let s3 = opts.s3.as_ref().map(resolve_s3).transpose()?;
    let adv = resolve_advanced(opts)?;

    Ok(ResolvedConfig {
        bind_host,
        port,
        http_port,
        store_dir,
        cert_file,
        pkey_file,
        auth: opts.auth,
        s3,
        adv,
    })
}

/// Build the resolved config + write the config file, returning everything
/// `spawn` needs. Stores live directly under `store_dir`; the config file goes
/// into `store_dir/.loregui-host/local.toml` so a single store directory is
/// fully self-describing.
fn prepare(opts: &HostServerOptions) -> Result<(ResolvedConfig, PathBuf, String), LoreError> {
    let cfg = resolve_config(opts, false)?;
    let store_dir = cfg.store_dir.clone();
    std::fs::create_dir_all(&store_dir).map_err(|e| {
        LoreError::CommandFailed(format!(
            "could not create store dir {}: {e}",
            store_dir.display()
        ))
    })?;

    let config_dir = store_dir.join(".loregui-host");
    std::fs::create_dir_all(&config_dir).map_err(|e| {
        LoreError::CommandFailed(format!(
            "could not create config dir {}: {e}",
            config_dir.display()
        ))
    })?;
    let config_path = config_dir.join("local.toml");
    std::fs::write(&config_path, render_config_toml(&cfg)).map_err(|e| {
        LoreError::CommandFailed(format!(
            "could not write config {}: {e}",
            config_path.display()
        ))
    })?;

    let url = advertise_url(cfg.port, opts.repository_name.as_deref());
    Ok((cfg, config_path, url))
}

/// Start a hosted server for the given options. Idempotent: if a server is
/// already running this returns an error rather than spawning a second one
/// (call stop first, or read status).
pub fn start(
    slot: &mut Option<HostedServer>,
    opts: &HostServerOptions,
) -> Result<HostStatus, LoreError> {
    if let Some(existing) = slot.as_mut() {
        // Reap if it died out from under us; otherwise refuse.
        match existing.child.as_mut().map(|c| c.try_wait()) {
            Some(Ok(Some(_))) | None => {
                // exited — fall through to (re)start
                *slot = None;
            }
            Some(Ok(None)) => {
                return Err(LoreError::CommandFailed(format!(
                    "a hosted server is already running (pid {}, {})",
                    existing.pid, existing.url
                )));
            }
            Some(Err(e)) => {
                return Err(LoreError::CommandFailed(format!(
                    "could not check existing server state: {e}"
                )));
            }
        }
    }

    let (cfg, config_path, url) = prepare(opts)?;
    let binary = resolve_server_binary()?;
    let config_dir = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    // Boot exactly like the spike: LORE_CONFIG_PATH points at the dir holding
    // local.toml, LORE_ENV=local selects it. cwd = config dir.
    let mut command = Command::new(&binary);
    command
        .env("LORE_CONFIG_PATH", &config_dir)
        .env("LORE_ENV", "local")
        .current_dir(&config_dir);

    // For an S3-backed (aws-mode) immutable store, lore resolves credentials via
    // the standard AWS credential chain — NOT from the TOML. Export the access
    // key + region as env vars on the child so the chain picks them up. (When
    // absent, the chain falls back to ambient AWS config / instance role, which
    // is the right behaviour for a host already authenticated to AWS.)
    if let Some(s3) = &cfg.s3 {
        if let Some(id) = &s3.access_key_id {
            command.env("AWS_ACCESS_KEY_ID", id);
        }
        if let Some(secret) = &s3.secret_access_key {
            command.env("AWS_SECRET_ACCESS_KEY", secret);
        }
        if let Some(region) = &s3.region {
            command.env("AWS_REGION", region);
            command.env("AWS_DEFAULT_REGION", region);
        }
    }

    let child = command.spawn().map_err(|e| {
        LoreError::CommandFailed(format!(
            "failed to launch loreserver ({}): {e}",
            binary.display()
        ))
    })?;

    let server = HostedServer {
        pid: child.id(),
        child: Some(child),
        port: cfg.port,
        http_port: cfg.http_port,
        url,
        config_path,
        store_dir: cfg.store_dir,
    };
    let status = HostStatus::from(&server);
    *slot = Some(server);
    Ok(status)
}

/// Stop the hosted server (kill + reap). Idempotent: a no-op if none running.
pub fn stop(slot: &mut Option<HostedServer>) -> Result<HostStatus, LoreError> {
    if let Some(mut server) = slot.take() {
        if let Some(mut child) = server.child.take() {
            // Best-effort: ignore "already exited" errors.
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    Ok(HostStatus::stopped())
}

/// Current status. Reaps the child if it has exited so status reflects reality.
pub fn status(slot: &mut Option<HostedServer>) -> HostStatus {
    let exited = match slot.as_mut() {
        Some(server) => match server.child.as_mut().map(|c| c.try_wait()) {
            Some(Ok(Some(_))) | None => true,
            Some(Ok(None)) => false,
            Some(Err(_)) => false,
        },
        None => false,
    };
    if exited {
        *slot = None;
    }
    match slot.as_ref() {
        Some(server) => HostStatus::from(server),
        None => HostStatus::stopped(),
    }
}

/// Render the `loreserver` config TOML for the given options **without** writing
/// anything to disk or starting a server (SBAI-4075). Backs the host flow's
/// "View generated config" affordance so an operator can review exactly what
/// will be written before committing. Validation errors (bad enum, out-of-range
/// number, required-when-mode) surface here too, so the preview doubles as a
/// dry-run check.
pub fn render_config(opts: &HostServerOptions) -> Result<String, LoreError> {
    // `allow_missing_certs = true`: a preview should work even before the lore
    // dependency has been fetched/built, so fall back to labelled placeholder
    // cert paths rather than erroring.
    let cfg = resolve_config(opts, true)?;
    Ok(render_config_toml(&cfg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_cfg(store: &str, port: u16, auth: bool) -> ResolvedConfig {
        ResolvedConfig {
            bind_host: BIND_HOST.to_owned(),
            port,
            http_port: port + 2,
            store_dir: PathBuf::from(store),
            cert_file: PathBuf::from("/certs/test_cert.pem"),
            pkey_file: PathBuf::from("/certs/test_key.pem"),
            auth,
            s3: None,
            adv: AdvancedConfig::default(),
        }
    }

    fn sample_s3_cfg(store: &str, port: u16, s3: ResolvedS3) -> ResolvedConfig {
        ResolvedConfig {
            bind_host: BIND_HOST.to_owned(),
            port,
            http_port: port + 2,
            store_dir: PathBuf::from(store),
            cert_file: PathBuf::from("/certs/test_cert.pem"),
            pkey_file: PathBuf::from("/certs/test_key.pem"),
            auth: false,
            s3: Some(s3),
            adv: AdvancedConfig::default(),
        }
    }

    /// A minimal [`HostServerOptions`] with only a store dir — the simple
    /// first-run case. Every advanced section is `None`.
    fn basic_opts(store: &str) -> HostServerOptions {
        HostServerOptions {
            store_dir: store.to_owned(),
            ..Default::default()
        }
    }

    /// Options for `store` with the given Expert-mode advanced bag.
    fn adv_opts(store: &str, advanced: HostAdvancedOptions) -> HostServerOptions {
        HostServerOptions {
            store_dir: store.to_owned(),
            advanced: Some(advanced),
            ..Default::default()
        }
    }

    #[test]
    fn config_has_required_sections_and_values() {
        let toml = render_config_toml(&sample_cfg("/srv/store", 41337, false));
        // localhost binds
        assert!(toml.contains("[server.quic]"));
        assert!(toml.contains("[server.grpc]"));
        assert!(toml.contains("[server.http]"));
        assert!(toml.contains("host = \"127.0.0.1\""));
        assert!(toml.contains("port = 41337"));
        // http is port + 2
        assert!(toml.contains("port = 41339"));
        // local stores point at the chosen dir
        assert!(toml.contains("[immutable_store.local]"));
        assert!(toml.contains("[mutable_store.local]"));
        assert!(toml.contains("path = \"/srv/store\""));
        // certs
        assert!(toml.contains("cert_file = \"/certs/test_cert.pem\""));
        assert!(toml.contains("pkey_file = \"/certs/test_key.pem\""));
        // single node
        assert!(toml.contains("[topology]"));
        assert!(toml.contains("provider = \"none\""));
    }

    #[test]
    fn config_is_auth_disabled_by_default() {
        let toml = render_config_toml(&sample_cfg("/srv/store", 41337, false));
        // No [server.auth] block → server runs auth-disabled (the key enabler).
        assert!(!toml.contains("[server.auth]"));
    }

    #[test]
    fn config_escapes_windows_paths() {
        let cfg = sample_cfg(r"C:\Users\dev\store", 50000, false);
        let toml = render_config_toml(&cfg);
        // Backslashes are doubled so the TOML basic string is valid.
        assert!(toml.contains(r#"path = "C:\\Users\\dev\\store""#));
    }

    #[test]
    fn local_config_does_not_emit_aws_store() {
        let toml = render_config_toml(&sample_cfg("/srv/store", 41337, false));
        // The default (no S3) path stays fully local: no aws mode, no plugin.
        assert!(!toml.contains("mode = \"aws\""));
        assert!(!toml.contains("[plugins.aws"));
        assert!(toml.contains("[immutable_store.local]"));
    }

    #[test]
    fn s3_config_emits_aws_immutable_store_and_local_mutable_store() {
        let s3 = ResolvedS3 {
            endpoint: Some("https://s3.us-west-2.amazonaws.com".into()),
            bucket: "lore-prod".into(),
            region: Some("us-west-2".into()),
            access_key_id: Some("AKIAEXAMPLE".into()),
            secret_access_key: Some("supersecret".into()),
            force_path_style: false,
            dynamodb_endpoint: None,
        };
        let toml = render_config_toml(&sample_s3_cfg("/srv/store", 41337, s3));

        // Immutable store is aws (S3) mode.
        assert!(toml.contains("[immutable_store]"));
        assert!(toml.contains("mode = \"aws\""));

        // aws immutable plugin: S3 payloads + DynamoDB metadata (tables derived
        // from the bucket and auto-ensured by lore).
        assert!(toml.contains("[plugins.aws.immutable_store]"));
        assert!(toml.contains("s3_bucket = \"lore-prod\""));
        assert!(toml.contains("s3_endpoint_url = \"https://s3.us-west-2.amazonaws.com\""));
        assert!(toml.contains("s3_region = \"us-west-2\""));
        assert!(toml.contains("s3_force_path_style = false"));
        assert!(toml.contains("dynamodb_fragments_table = \"lore-prod-fragments\""));
        assert!(toml.contains("dynamodb_metadata_table = \"lore-prod-fragment-metadata\""));
        assert!(toml.contains("dynamodb_region = \"us-west-2\""));

        // Mutable (branch-pointer) store stays local — see render_config_toml docs.
        assert!(toml.contains("[mutable_store.local]"));
        assert!(toml.contains("path = \"/srv/store\""));
        // ...and is NOT switched to aws.
        assert!(!toml.contains("[mutable_store]\nmode"));

        // Secrets are never written into the config TOML (they go via env vars).
        assert!(!toml.contains("AKIAEXAMPLE"));
        assert!(!toml.contains("supersecret"));
    }

    #[test]
    fn s3_config_for_minio_garage_uses_path_style_and_custom_endpoint() {
        // MinIO/Garage: custom endpoint + path-style + a DynamoDB-compatible
        // endpoint for the metadata tables.
        let s3 = ResolvedS3 {
            endpoint: Some("http://127.0.0.1:9000".into()),
            bucket: "lore".into(),
            region: Some("garage".into()),
            access_key_id: None,
            secret_access_key: None,
            force_path_style: true,
            dynamodb_endpoint: Some("http://127.0.0.1:8000".into()),
        };
        let toml = render_config_toml(&sample_s3_cfg("/srv/store", 41337, s3));
        assert!(toml.contains("s3_endpoint_url = \"http://127.0.0.1:9000\""));
        assert!(toml.contains("s3_force_path_style = true"));
        assert!(toml.contains("dynamodb_endpoint_url = \"http://127.0.0.1:8000\""));
    }

    #[test]
    fn s3_config_omits_optional_keys_when_absent() {
        // Real AWS S3 with ambient creds: no endpoint, no region, no path-style.
        let s3 = ResolvedS3 {
            endpoint: None,
            bucket: "lore".into(),
            region: None,
            access_key_id: None,
            secret_access_key: None,
            force_path_style: false,
            dynamodb_endpoint: None,
        };
        let toml = render_config_toml(&sample_s3_cfg("/srv/store", 41337, s3));
        assert!(toml.contains("s3_bucket = \"lore\""));
        // Optional keys are omitted entirely (SDK resolves defaults).
        assert!(!toml.contains("s3_endpoint_url"));
        assert!(!toml.contains("s3_region"));
        assert!(!toml.contains("dynamodb_endpoint_url"));
        assert!(!toml.contains("dynamodb_region"));
        // Required DynamoDB table names are always present.
        assert!(toml.contains("dynamodb_fragments_table = \"lore-fragments\""));
        assert!(toml.contains("dynamodb_metadata_table = \"lore-fragment-metadata\""));
    }

    #[test]
    fn resolve_s3_trims_blanks_and_requires_bucket() {
        // Blank bucket → error.
        let blank = S3StoreOptions {
            endpoint: Some("  ".into()),
            bucket: "   ".into(),
            region: None,
            access_key_id: None,
            secret_access_key: None,
            force_path_style: false,
            dynamodb_endpoint: None,
        };
        assert!(resolve_s3(&blank).is_err());

        // Blanks elsewhere normalise to None; bucket is trimmed.
        let opts = S3StoreOptions {
            endpoint: Some("  https://s3.example.com  ".into()),
            bucket: "  my-bucket  ".into(),
            region: Some("".into()),
            access_key_id: Some("  key  ".into()),
            secret_access_key: None,
            force_path_style: true,
            dynamodb_endpoint: None,
        };
        let resolved = resolve_s3(&opts).expect("valid");
        assert_eq!(resolved.bucket, "my-bucket");
        assert_eq!(resolved.endpoint.as_deref(), Some("https://s3.example.com"));
        assert_eq!(resolved.region, None);
        assert_eq!(resolved.access_key_id.as_deref(), Some("key"));
        assert!(resolved.force_path_style);
        assert_eq!(resolved.fragments_table(), "my-bucket-fragments");
        assert_eq!(resolved.metadata_table(), "my-bucket-fragment-metadata");
    }

    #[test]
    fn advertise_url_with_and_without_repo() {
        assert_eq!(
            advertise_url(41337, Some("myrepo")),
            "lore://127.0.0.1:41337/myrepo"
        );
        assert_eq!(advertise_url(41337, None), "lore://127.0.0.1:41337");
        // blank/whitespace repo name → bare URL
        assert_eq!(advertise_url(41337, Some("  ")), "lore://127.0.0.1:41337");
    }

    #[test]
    fn parse_pinned_rev_finds_40_hex() {
        let toml = r#"
            lore = { git = "https://github.com/EpicGames/lore.git", rev = "65598412872a15685e1e8cd6d9d88425eedbc3c2" }
        "#;
        assert_eq!(
            parse_pinned_rev(toml).as_deref(),
            Some("65598412872a15685e1e8cd6d9d88425eedbc3c2")
        );
        assert_eq!(parse_pinned_rev("rev = \"short\""), None);
    }

    // === Advanced config (SBAI-4075) ===

    /// The most important invariant: an all-default `HostServerOptions` (the
    /// simple first-run case) must render the **exact same** local config the
    /// flow always produced — same sections, same values, no advanced keys.
    #[test]
    fn defaults_render_the_original_local_config() {
        // What `render_config` produces for a bare store dir...
        let opts = basic_opts("/srv/store");
        let adv = resolve_advanced(&opts).expect("no advanced options");
        assert!(
            matches!(
                adv,
                AdvancedConfig {
                    quic: None,
                    grpc: None,
                    http: None,
                    local_store: None,
                    topology: None,
                    telemetry: None,
                    runtime: None,
                    notification: None,
                    features: None,
                    timeouts: None,
                    quic_internal: None,
                    replication_endpoint: None,
                    lock_store_mode: None
                }
            ),
            "a bare options bag must resolve to a fully-empty AdvancedConfig"
        );

        let cfg = ResolvedConfig {
            bind_host: BIND_HOST.to_owned(),
            port: DEFAULT_PORT,
            http_port: DEFAULT_PORT + 2,
            store_dir: PathBuf::from("/srv/store"),
            cert_file: PathBuf::from("/certs/test_cert.pem"),
            pkey_file: PathBuf::from("/certs/test_key.pem"),
            auth: false,
            s3: None,
            adv,
        };
        let toml = render_config_toml(&cfg);

        // The minimal config body, unchanged from the SBAI-4065 baseline: quic +
        // certs, grpc, http, local stores, telemetry logger ansi, topology none.
        // No advanced sections leak in when nothing was set.
        assert!(toml.contains("[server.quic]\nhost = \"127.0.0.1\"\nport = 41337\n[server.quic.certificate]\ncert_file = \"/certs/test_cert.pem\"\npkey_file = \"/certs/test_key.pem\"\n"));
        assert!(toml.contains("[server.grpc]\nhost = \"127.0.0.1\"\nport = 41337\n"));
        assert!(toml.contains("[server.http]\nhost = \"127.0.0.1\"\nport = 41339\n"));
        assert!(toml.contains("[immutable_store.local]\npath = \"/srv/store\"\n"));
        assert!(toml.contains("[mutable_store.local]\npath = \"/srv/store\"\n"));
        assert!(toml.contains("[telemetry.logger]\nformat = \"ansi\"\n"));
        assert!(toml.contains("[topology]\nprovider = \"none\"\n"));
        // No advanced sections.
        assert!(!toml.contains("[server.quic_internal]"));
        assert!(!toml.contains("[server.replication]"));
        assert!(!toml.contains("[tokio]"));
        assert!(!toml.contains("[feature]"));
        assert!(!toml.contains("[notification]"));
        assert!(!toml.contains("[lock_store]"));
        assert!(!toml.contains("[telemetry.metrics]"));
        // No per-endpoint tuning keys when unset.
        assert!(!toml.contains("verify_client_certs"));
        assert!(!toml.contains("idle_timeout"));
        assert!(!toml.contains("max_file_size"));
    }

    #[test]
    fn bind_host_override_applies_to_every_endpoint() {
        let opts = HostServerOptions {
            store_dir: "/srv/store".into(),
            bind_host: Some("0.0.0.0".into()),
            ..Default::default()
        };
        let cfg = resolve_config(&opts, true).expect("resolve");
        let toml = render_config_toml(&cfg);
        // All three endpoints bind the override; loopback no longer appears.
        assert_eq!(toml.matches("host = \"0.0.0.0\"").count(), 3);
        assert!(!toml.contains("host = \"127.0.0.1\""));
    }

    #[test]
    fn quic_grpc_http_overrides_are_emitted() {
        let opts = adv_opts(
            "/srv/store",
            HostAdvancedOptions {
                quic: Some(QuicOptions {
                    verify_client_certs: Some(true),
                    idle_timeout: Some(60_000),
                    num_listeners: Some(4),
                    handler_timeout_seconds: Some(30),
                    ..Default::default()
                }),
                grpc: Some(GrpcOptions {
                    request_handler_timeout_seconds: Some(45),
                    http2_keepalive_interval_seconds: Some(20),
                    ..Default::default()
                }),
                http: Some(HttpOptions {
                    max_file_size: Some(20_971_520),
                    store_health_check: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        let toml = render_config(&opts).expect("render");
        assert!(toml.contains("[server.quic]"));
        assert!(toml.contains("verify_client_certs = true"));
        assert!(toml.contains("idle_timeout = 60000"));
        assert!(toml.contains("num_listeners = 4"));
        assert!(toml.contains("handler_timeout_seconds = 30"));
        assert!(toml.contains("request_handler_timeout_seconds = 45"));
        assert!(toml.contains("http2_keepalive_interval_seconds = 20"));
        assert!(toml.contains("max_file_size = 20971520"));
        assert!(toml.contains("store_health_check = true"));
    }

    #[test]
    fn telemetry_runtime_notification_feature_sections_render() {
        let opts = adv_opts(
            "/srv/store",
            HostAdvancedOptions {
                telemetry: Some(TelemetryOptions {
                    log_format: Some("json".into()),
                    log_output: Some("file".into()),
                    log_file: Some("/var/log/lore.log".into()),
                    enable_otlp: Some(true),
                    metrics_export_interval_millis: Some(15_000),
                    trace_sample_rate: Some(0.5),
                    ..Default::default()
                }),
                runtime: Some(RuntimeOptions {
                    worker_threads: Some(8),
                    max_blocking_threads: Some(256),
                    ..Default::default()
                }),
                notification: Some(NotificationOptions {
                    mode: Some("local".into()),
                }),
                features: Some(FeatureOptions {
                    history_step_size: Some(50),
                    revision_step_keys: Some(false),
                    ..Default::default()
                }),
                timeouts: Some(TimeoutOptions {
                    connection_close_timeout_seconds: Some(10),
                    runtime_shutdown_timeout_seconds: Some(40),
                }),
                lock_store_mode: Some("local".into()),
                ..Default::default()
            },
        );
        let toml = render_config(&opts).expect("render");
        // telemetry
        assert!(toml.contains("[telemetry.logger]\nformat = \"json\""));
        assert!(toml.contains("output = { file = \"/var/log/lore.log\" }"));
        assert!(toml.contains("enable_otlp = true"));
        assert!(toml.contains("[telemetry.metrics]\nexport_interval_millis = 15000"));
        assert!(toml.contains("[telemetry.traces]\nsample_rate = 0.5"));
        // runtime
        assert!(toml.contains("[tokio]\nworker_threads = 8\nmax_blocking_threads = 256"));
        // notification + lock store + timeouts + feature
        assert!(toml.contains("[notification]\nmode = \"local\""));
        assert!(toml.contains("[lock_store]\nmode = \"local\""));
        assert!(toml.contains("connection_close_timeout_seconds = 10"));
        assert!(toml.contains("runtime_shutdown_timeout_seconds = 40"));
        assert!(toml.contains("[feature]\nhistory_step_size = 50\nrevision_step_keys = false"));
    }

    #[test]
    fn fixed_topology_renders_peers() {
        let opts = adv_opts(
            "/srv/store",
            HostAdvancedOptions {
                topology: Some(TopologyOptions {
                    provider: Some("fixed".into()),
                    peers: vec![
                        PeerOption {
                            address: "10.0.0.1".into(),
                            port: 41337,
                            locality: Some("SameRegion".into()),
                        },
                        PeerOption {
                            address: "10.0.0.2".into(),
                            port: 41337,
                            locality: Some("OtherRegion".into()),
                        },
                    ],
                    rotation_interval_seconds: None,
                }),
                ..Default::default()
            },
        );
        let toml = render_config(&opts).expect("render");
        assert!(toml.contains("[topology]\nprovider = \"fixed\""));
        assert!(toml.contains("[[topology.fixed.peers]]\naddress = \"10.0.0.1\"\nport = 41337\nlocality = \"SameRegion\""));
        assert!(toml.contains("address = \"10.0.0.2\""));
        assert!(toml.contains("locality = \"OtherRegion\""));
    }

    #[test]
    fn rotating_topology_requires_interval() {
        let mut opts = adv_opts(
            "/srv/store",
            HostAdvancedOptions {
                topology: Some(TopologyOptions {
                    provider: Some("rotating_id_fixed".into()),
                    peers: vec![PeerOption {
                        address: "10.0.0.1".into(),
                        port: 41337,
                        locality: None,
                    }],
                    rotation_interval_seconds: None,
                }),
                ..Default::default()
            },
        );
        // Missing interval → error.
        assert!(render_config(&opts).is_err());
        // With interval → renders the rotating section + peer.
        opts.advanced
            .as_mut()
            .unwrap()
            .topology
            .as_mut()
            .unwrap()
            .rotation_interval_seconds = Some(3600);
        let toml = render_config(&opts).expect("render");
        assert!(toml.contains("provider = \"rotating_id_fixed\""));
        assert!(toml.contains("[topology.rotating_id_fixed]\nrotation_interval_seconds = 3600"));
        assert!(toml.contains("[[topology.rotating_id_fixed.peers]]"));
    }

    #[test]
    fn internal_endpoint_requires_certs_when_enabled() {
        // enabled but no certs → error.
        let bad = adv_opts(
            "/srv/store",
            HostAdvancedOptions {
                quic_internal: Some(InternalEndpointOptions {
                    enabled: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        assert!(render_config(&bad).is_err());

        // enabled with certs → renders the section + nested certificate table.
        let ok = adv_opts(
            "/srv/store",
            HostAdvancedOptions {
                replication_endpoint: Some(InternalEndpointOptions {
                    enabled: Some(true),
                    port: Some(41340),
                    cert_file: Some("/c/cert.pem".into()),
                    pkey_file: Some("/c/key.pem".into()),
                    cert_chain: None,
                }),
                ..Default::default()
            },
        );
        let toml = render_config(&ok).expect("render");
        assert!(toml.contains("[server.replication]\nenabled = true\nport = 41340"));
        assert!(toml.contains("[server.replication.certificate]\ncert_file = \"/c/cert.pem\"\npkey_file = \"/c/key.pem\""));
    }

    #[test]
    fn validation_rejects_bad_enums_and_ranges() {
        let bad_format = adv_opts(
            "/s",
            HostAdvancedOptions {
                telemetry: Some(TelemetryOptions {
                    log_format: Some("yaml".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        assert!(render_config(&bad_format).is_err());

        let bad_rate = adv_opts(
            "/s",
            HostAdvancedOptions {
                telemetry: Some(TelemetryOptions {
                    trace_sample_rate: Some(1.5),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        assert!(render_config(&bad_rate).is_err());

        let bad_provider = adv_opts(
            "/s",
            HostAdvancedOptions {
                topology: Some(TopologyOptions {
                    provider: Some("consul".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        assert!(render_config(&bad_provider).is_err());
    }

    #[test]
    fn render_config_preview_does_not_require_store_dir_on_disk() {
        // A non-existent path must still render (preview writes nothing to disk).
        let opts = basic_opts("/nonexistent/preview/only/store");
        let toml = render_config(&opts).expect("preview should render");
        assert!(toml.contains("provider = \"none\""));
        // Empty store dir is still rejected.
        let empty = basic_opts("   ");
        assert!(render_config(&empty).is_err());
    }

    #[test]
    fn s3_advanced_keeps_local_mutable_tuning() {
        // S3 immutable + local-store flush override only touches the mutable
        // local store; immutable-only keys (compaction etc.) are not emitted.
        let opts = HostServerOptions {
            store_dir: "/srv/store".into(),
            s3: Some(S3StoreOptions {
                endpoint: None,
                bucket: "lore".into(),
                region: Some("us-east-1".into()),
                access_key_id: None,
                secret_access_key: None,
                force_path_style: false,
                dynamodb_endpoint: None,
            }),
            advanced: Some(HostAdvancedOptions {
                local_store: Some(LocalStoreOptions {
                    flush_delay_seconds: Some(20),
                    compaction_delay: Some(99),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let toml = render_config(&opts).expect("render");
        assert!(toml.contains("mode = \"aws\""));
        assert!(toml.contains("[mutable_store.local]"));
        assert!(toml.contains("flush_delay_seconds = 20"));
        // compaction_delay is immutable-only; the immutable store is aws here, so
        // it must NOT appear.
        assert!(!toml.contains("compaction_delay"));
    }

    /// Live smoke test (LOCAL-ONLY, ignored by default): actually spawn a real
    /// `loreserver` via `start()` and prove it boots + binds its gRPC/QUIC port,
    /// then `stop()` reaps it. Launches the upstream server binary (resolved from
    /// the dev checkout), so run only on a dev box:
    ///   cargo test -p loregui --lib server_host::tests::live_ -- --ignored --nocapture
    #[test]
    #[ignore = "live: spawns the real loreserver; local dev box only"]
    fn live_host_server_boots_binds_and_stops() {
        use std::net::TcpStream;
        use std::time::{Duration, Instant};

        let store = std::env::temp_dir().join(format!("loregui-host-smoke-{}", std::process::id()));
        std::fs::create_dir_all(&store).unwrap();
        let port = 41355u16;
        let mut slot: Option<HostedServer> = None;
        let opts = HostServerOptions {
            store_dir: store.to_string_lossy().into_owned(),
            port: Some(port),
            repository_name: Some("smoke".into()),
            ..Default::default()
        };

        let started = start(&mut slot, &opts).expect("start should spawn loreserver");
        assert!(started.running, "status should report running after start");
        assert_eq!(started.url.as_deref(), Some("lore://127.0.0.1:41355/smoke"));

        // gRPC binds TCP on `port` — poll until it accepts a connection.
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut bound = false;
        while Instant::now() < deadline {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                bound = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        let st = status(&mut slot);
        if !bound {
            let _ = stop(&mut slot);
            let _ = std::fs::remove_dir_all(&store);
            panic!(
                "loreserver did not bind 127.0.0.1:{port} within 30s (running={})",
                st.running
            );
        }
        assert!(st.running, "status should still be running once bound");

        let stopped = stop(&mut slot).expect("stop");
        assert!(
            !stopped.running,
            "status should report stopped after stop()"
        );
        let _ = std::fs::remove_dir_all(&store);
    }
}
