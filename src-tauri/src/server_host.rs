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
}

/// Render the `loreserver` config TOML from resolved inputs.
///
/// Pure and deterministic so it can be unit-tested. Mirrors the spike's
/// `local.toml`: localhost QUIC + gRPC on the same port number (TCP gRPC / UDP
/// QUIC), HTTP on `port + 2`, local immutable + mutable stores under the chosen
/// directory, the shipped test certs for QUIC, single-node topology, and —
/// crucially — **no `[server.auth]` block** so the server runs auth-disabled.
fn render_config_toml(cfg: &ResolvedConfig) -> String {
    // Paths are emitted as TOML basic strings; escape backslashes (Windows) and
    // quotes so the file is valid regardless of the platform's path separators.
    let esc = |p: &Path| -> String {
        p.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    };

    let mut out = String::new();
    out.push_str("# Generated by LoreGUI \"Host a server\" (SBAI-4065). Do not edit by hand.\n");
    out.push_str("# Single-node, loopback-only, local-store loreserver config.\n\n");

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

    out.push_str("[immutable_store.local]\n");
    out.push_str(&format!("path = \"{}\"\n", esc(&cfg.store_dir)));
    out.push_str("[mutable_store.local]\n");
    out.push_str(&format!("path = \"{}\"\n\n", esc(&cfg.store_dir)));

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

    let cfg = ResolvedConfig {
        port,
        http_port,
        store_dir: store_dir.clone(),
        cert_file,
        pkey_file,
        auth: opts.auth,
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
    let child = Command::new(&binary)
        .env("LORE_CONFIG_PATH", &config_dir)
        .env("LORE_ENV", "local")
        .current_dir(&config_dir)
        .spawn()
        .map_err(|e| {
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
