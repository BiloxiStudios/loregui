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
#[derive(Debug, Clone, Deserialize)]
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

    let mut out = String::new();
    out.push_str("# Generated by LoreGUI \"Host a server\" (SBAI-4065). Do not edit by hand.\n");
    if cfg.s3.is_some() {
        out.push_str(
            "# Single-node, loopback-only loreserver: S3-compatible immutable store + local mutable store.\n\n",
        );
    } else {
        out.push_str("# Single-node, loopback-only, local-store loreserver config.\n\n");
    }

    out.push_str("[server.quic]\n");
    out.push_str(&format!("host = \"{BIND_HOST}\"\n"));
    out.push_str(&format!("port = {}\n", cfg.port));
    out.push_str("[server.quic.certificate]\n");
    out.push_str(&format!("cert_file = \"{}\"\n", esc(&cfg.cert_file)));
    out.push_str(&format!("pkey_file = \"{}\"\n\n", esc(&cfg.pkey_file)));

    out.push_str("[server.grpc]\n");
    out.push_str(&format!("host = \"{BIND_HOST}\"\n"));
    out.push_str(&format!("port = {}\n\n", cfg.port));

    out.push_str("[server.http]\n");
    out.push_str(&format!("host = \"{BIND_HOST}\"\n"));
    out.push_str(&format!("port = {}\n\n", cfg.http_port));

    match &cfg.s3 {
        // Local filesystem immutable + mutable stores (the default).
        None => {
            out.push_str("[immutable_store.local]\n");
            out.push_str(&format!("path = \"{}\"\n", esc(&cfg.store_dir)));
            out.push_str("[mutable_store.local]\n");
            out.push_str(&format!("path = \"{}\"\n\n", esc(&cfg.store_dir)));
        }
        // S3-compatible (lore `aws` mode) immutable store; local mutable store.
        Some(s3) => {
            out.push_str("[immutable_store]\n");
            out.push_str("mode = \"aws\"\n\n");

            // Mutable (branch pointers) stays on local disk — see fn docs.
            out.push_str("[mutable_store.local]\n");
            out.push_str(&format!("path = \"{}\"\n\n", esc(&cfg.store_dir)));

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

    out.push_str("[telemetry.logger]\n");
    out.push_str("format = \"ansi\"\n\n");

    out.push_str("[topology]\n");
    out.push_str("provider = \"none\"\n");

    // Auth hook: a future authed mode would append a `[server.auth]` block here.
    // The no-auth local host flow deliberately omits it (server logs
    // "Auth: disabled"). Keep the branch explicit so the intent is documented.
    if cfg.auth {
        out.push_str("\n# NOTE: authed hosting is not yet implemented; running auth-disabled.\n");
    }

    // FOLLOW-UP (advanced / enterprise lore store modes, deferred — see
    // docs/domains/storage.md): lore also supports a `composite` immutable store
    // (local cache tier + durable `aws`/S3 tier with a `ReplicationMode` of
    // read/write/read_write), a `replicated` store (server-to-server QUIC), and
    // an `aws` (DynamoDB) mutable + lock store at scale. They need extra inputs
    // (replica peers, DynamoDB tables/region, mTLS replication certs) the
    // first-run host wizard does not collect, so they are intentionally not
    // emitted here. Add new `ResolvedConfig` variants + render branches to wire
    // them when those flows land.

    out
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
    let norm = |s: &Option<String>| -> Option<String> {
        s.as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    };

    let bucket = opts.bucket.trim();
    if bucket.is_empty() {
        return Err(LoreError::CommandFailed(
            "an S3-compatible bucket name is required to host with object storage".into(),
        ));
    }

    Ok(ResolvedS3 {
        endpoint: norm(&opts.endpoint),
        bucket: bucket.to_owned(),
        region: norm(&opts.region),
        access_key_id: norm(&opts.access_key_id),
        secret_access_key: norm(&opts.secret_access_key),
        force_path_style: opts.force_path_style,
        dynamodb_endpoint: norm(&opts.dynamodb_endpoint),
    })
}

/// Build the resolved config + write the config file, returning everything
/// `spawn` needs. Stores live directly under `store_dir`; the config file goes
/// into `store_dir/.loregui-host/local.toml` so a single store directory is
/// fully self-describing.
fn prepare(opts: &HostServerOptions) -> Result<(ResolvedConfig, PathBuf, String), LoreError> {
    let store_dir = PathBuf::from(opts.store_dir.trim());
    if store_dir.as_os_str().is_empty() {
        return Err(LoreError::CommandFailed(
            "store directory is required to host a server".into(),
        ));
    }
    std::fs::create_dir_all(&store_dir).map_err(|e| {
        LoreError::CommandFailed(format!(
            "could not create store dir {}: {e}",
            store_dir.display()
        ))
    })?;

    let port = match opts.port {
        Some(p) if p != 0 => p,
        _ => DEFAULT_PORT,
    };
    let http_port = port.wrapping_add(2);

    let checkout = lore_checkout()?;
    let test_data = checkout
        .join("lore-server")
        .join("src")
        .join("protocol")
        .join("test_data");
    let cert_file = test_data.join("test_cert.pem");
    let pkey_file = test_data.join("test_key.pem");

    let s3 = opts.s3.as_ref().map(resolve_s3).transpose()?;

    let cfg = ResolvedConfig {
        port,
        http_port,
        store_dir: store_dir.clone(),
        cert_file,
        pkey_file,
        auth: opts.auth,
        s3,
    };

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

    let url = advertise_url(port, opts.repository_name.as_deref());
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_cfg(store: &str, port: u16, auth: bool) -> ResolvedConfig {
        ResolvedConfig {
            port,
            http_port: port + 2,
            store_dir: PathBuf::from(store),
            cert_file: PathBuf::from("/certs/test_cert.pem"),
            pkey_file: PathBuf::from("/certs/test_key.pem"),
            auth,
            s3: None,
        }
    }

    fn sample_s3_cfg(store: &str, port: u16, s3: ResolvedS3) -> ResolvedConfig {
        ResolvedConfig {
            port,
            http_port: port + 2,
            store_dir: PathBuf::from(store),
            cert_file: PathBuf::from("/certs/test_cert.pem"),
            pkey_file: PathBuf::from("/certs/test_key.pem"),
            auth: false,
            s3: Some(s3),
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
            auth: false,
            s3: None,
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
