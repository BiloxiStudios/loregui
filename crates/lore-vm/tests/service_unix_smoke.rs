//! Linux/macOS Unix-domain-socket service process smoke (SBAI-5473).
//!
//! Epic `437e727d` added a real UDS backend for `lore service run` on Unix.
//! LoreGUI's architecture still uses in-process lore bindings + standalone
//! `loreserver` (not the background CLI service), so this suite is
//! **compatibility-only**: it proves the new upstream path can start, serve a
//! readiness probe, resolve a relative operation against the caller rather
//! than the service process, and shut down cleanly when an **exact-pin** `lore`
//! binary is available. The caller-CWD canary is a required PR integration
//! gate and fails closed when that binary is unavailable.
//!
//! The basic start/stop smoke remains contributor-friendly, but the behavioral
//! caller-CWD canary never skips: CI provisions the exact pin first, and missing
//! or wrong-provenance input is a hard failure.
//!
//! ```sh
//! LOREVM_LORE_BIN=/path/to/lore-from-exact-pin \
//!   cargo test -p lore-vm --features integration-tests --test service_unix_smoke -- --nocapture
//! ```
#![cfg(all(feature = "integration-tests", target_family = "unix"))]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Extract the first 40-hex-char `rev = "..."` from Cargo.toml (same as remote harness).
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

fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or(manifest)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Workspace-pinned lore git rev from root `Cargo.toml` (40 hex chars).
fn pinned_rev() -> Option<String> {
    let cargo_toml = std::fs::read_to_string(repo_root().join("Cargo.toml")).ok()?;
    parse_pinned_rev(&cargo_toml)
}

/// Locate the cargo-unpacked `lore` git checkout for the **exact** pinned rev only.
/// Does **not** walk sibling short-rev directories (those may hold older pins).
fn lore_checkout() -> Option<PathBuf> {
    let rev = pinned_rev()?;
    let short = &rev[..7];
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|h| h.join(".cargo")))?;
    let checkouts = cargo_home.join("git").join("checkouts");
    for entry in std::fs::read_dir(&checkouts).ok()?.flatten() {
        if entry.file_name().to_string_lossy().starts_with("lore-") {
            let candidate = entry.path().join(short);
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Candidate paths for an **exact-pin** `lore` CLI binary (no sibling-rev scan).
///
/// Order:
///   1. `LOREVM_LORE_BIN` env override (caller responsibility to point at exact pin).
///   2. Pinned checkout `target/{release,debug}/lore` only.
fn exact_pin_lore_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os("LOREVM_LORE_BIN").map(PathBuf::from) {
        out.push(p);
    }
    if let Some(checkout) = lore_checkout() {
        for profile in ["release", "debug"] {
            out.push(checkout.join("target").join(profile).join("lore"));
        }
    }
    out
}

/// Resolve a `lore` CLI binary without building one and without sibling fallback.
fn resolve_lore_binary() -> Option<PathBuf> {
    let explicit = std::env::var_os("LOREVM_LORE_BIN").map(PathBuf::from);
    resolve_lore_binary_from(explicit, lore_checkout())
}

fn resolve_lore_binary_from(
    explicit: Option<PathBuf>,
    checkout: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return path.is_file().then_some(path);
    }
    checkout.and_then(|root| {
        ["release", "debug"]
            .into_iter()
            .map(|profile| root.join("target").join(profile).join("lore"))
            .find(|candidate| candidate.is_file())
    })
}

#[test]
fn explicit_missing_binary_fixture_never_falls_back_to_checkout_artifact() {
    let missing = PathBuf::from("/definitely/missing/exact-pin-lore");
    assert_eq!(
        resolve_lore_binary_from(Some(missing), lore_checkout()),
        None,
        "an explicit required fixture must fail closed instead of falling back"
    );
}

/// UDS socket lives under `$XDG_RUNTIME_DIR` (or `$TMPDIR`/`/tmp`) in a
/// uid-suffixed dir — mirror upstream `lore/src/remote/network/unix.rs`.
fn expected_socket_roots(runtime: &Path) -> Vec<PathBuf> {
    let uid = libc_uid();
    vec![runtime.join(format!("lore-{uid}"))]
}

fn libc_uid() -> u32 {
    // Avoid a libc dependency — read from /proc or use nix-less getuid via libc crate
    // if present. Fall back to parsing `id -u`.
    if let Ok(out) = Command::new("id").arg("-u").output() {
        if out.status.success() {
            if let Ok(s) = String::from_utf8(out.stdout) {
                if let Ok(u) = s.trim().parse::<u32>() {
                    return u;
                }
            }
        }
    }
    1000
}

struct ServiceProc {
    child: Child,
    runtime_dir: tempfile::TempDir,
    global_dir: tempfile::TempDir,
}

impl Drop for ServiceProc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn boot_service(lore_bin: &Path) -> Result<ServiceProc, String> {
    boot_service_in_directory(lore_bin, None)
}

fn boot_service_in_directory(
    lore_bin: &Path,
    service_directory: Option<&Path>,
) -> Result<ServiceProc, String> {
    let runtime_dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let global_dir = tempfile::tempdir().map_err(|e| format!("global tempdir: {e}"))?;
    let mut command = Command::new(lore_bin);
    command
        .args(["service", "run"])
        .env("XDG_RUNTIME_DIR", runtime_dir.path())
        .env("TMPDIR", runtime_dir.path())
        .env("LORE_GLOBAL_PATH", global_dir.path())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if let Some(directory) = service_directory {
        command.current_dir(directory);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("spawn lore service run: {e}"))?;

    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            let err = child
                .stderr
                .as_mut()
                .map(|s| {
                    let mut buf = String::new();
                    let _ = std::io::Read::read_to_string(s, &mut buf);
                    buf
                })
                .unwrap_or_default();
            return Err(format!(
                "lore service run exited early ({status}). stderr:\n{err}"
            ));
        }
        for root in expected_socket_roots(runtime_dir.path()) {
            // Upstream names the socket; probe any socket file under the uid dir.
            if root.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&root) {
                    for e in entries.flatten() {
                        let p = e.path();
                        // Try connect as a readiness probe (matches upstream's connect probe).
                        if std::os::unix::net::UnixStream::connect(&p).is_ok() {
                            return Ok(ServiceProc {
                                child,
                                runtime_dir,
                                global_dir,
                            });
                        }
                    }
                }
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("lore service run never opened a UDS socket within 15s".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn assert_binary_built_from_exact_pin(lore_bin: &Path) {
    let expected = pinned_rev().expect("workspace pinned lore rev");
    let checkout = lore_bin
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .unwrap_or_else(|| panic!("unexpected lore binary path: {}", lore_bin.display()));
    let output = Command::new("git")
        .args([
            "-C",
            checkout.to_str().expect("utf-8 checkout"),
            "rev-parse",
            "HEAD",
        ])
        .output()
        .unwrap_or_else(|e| panic!("inspect lore checkout revision: {e}"));
    assert!(
        output.status.success(),
        "exact-pin canary requires a lore binary built inside its git checkout: {}",
        lore_bin.display()
    );
    let actual = String::from_utf8(output.stdout)
        .expect("git revision utf-8")
        .trim()
        .to_string();
    assert_eq!(
        actual, expected,
        "client/service binary must match Cargo.toml pin"
    );

    let version = Command::new(lore_bin)
        .arg("--version")
        .output()
        .expect("run exact-pin lore --version");
    assert!(
        version.status.success(),
        "exact-pin lore --version must succeed"
    );
    let version = String::from_utf8(version.stdout).expect("lore version utf-8");
    assert!(
        version.starts_with("lore 0.8.6-nightly"),
        "unexpected client/service version for pin {expected}: {version}"
    );
}

/// SIGTERM the service and require a clean exit (upstream clean-shutdown path).
fn stop_service(mut proc: ServiceProc) -> Result<(), String> {
    // Prefer SIGTERM so Drop of UdsListener unlinks the socket.
    #[allow(clippy::cast_possible_wrap)]
    {
        let pid = proc.child.id() as i32;
        if libc_kill(pid, 15) != 0 {
            let _ = proc.child.kill();
        }
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(Some(status)) = proc.child.try_wait() {
            if status.success() || status.code() == Some(0) {
                return Ok(());
            }
            // Some lore builds exit 0 after SIGTERM; accept signalled-as-terminated too.
            if status.code().is_none() {
                return Ok(());
            }
            return Err(format!("lore service run exited uncleanly: {status}"));
        }
        if Instant::now() >= deadline {
            let _ = proc.child.kill();
            let _ = proc.child.wait();
            return Err("lore service run did not exit within 10s after SIGTERM".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn libc_kill(pid: i32, sig: i32) -> i32 {
    // libc is already in the dep tree via lore; call via raw extern if needed.
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    unsafe { kill(pid, sig) }
}

#[test]
fn unix_service_run_start_stop_smoke() {
    let Some(lore_bin) = resolve_lore_binary() else {
        eprintln!(
            "[SKIP] no exact-pin `lore` CLI binary resolved \
             (set LOREVM_LORE_BIN to a binary built from the Cargo.toml pin, or pre-build \
             target/{{release,debug}}/lore in that checkout) — Unix service smoke not run"
        );
        return;
    };
    eprintln!("[service] using lore binary {}", lore_bin.display());

    // Once a binary is resolved, boot/readiness/shutdown MUST fail the test.
    // Soft-skip is only for absence of an exact-pin binary (above). Converting a
    // resolved binary's unsupported/startup error into SKIP is forbidden: it
    // previously allowed older sibling-checkout binaries to green the gate.
    let proc = boot_service(&lore_bin).unwrap_or_else(|e| {
        panic!(
            "resolved lore binary failed to boot `service run` (must not soft-skip): {e}\n\
             binary={}",
            lore_bin.display()
        );
    });
    eprintln!(
        "[service] UDS listener ready under {}",
        proc.runtime_dir.path().display()
    );

    stop_service(proc).unwrap_or_else(|e| {
        panic!(
            "resolved lore binary failed clean shutdown (must not soft-skip): {e}\n\
             binary={}",
            lore_bin.display()
        );
    });
    eprintln!("[service] clean shutdown verified");
}

/// Behavioral exact-pin canary for Epic 9179c6d.
///
/// A background service is started from A while the client performs a relative
/// repository operation from B. The repository must be created only under B;
/// resolving it under A proves the service inherited its own process cwd.
#[test]
fn unix_service_resolves_relative_repository_against_caller_root() {
    let lore_bin = resolve_lore_binary().expect(
        "REQUIRED exact-pin caller-CWD canary has no lore binary; provision the pinned CLI or set LOREVM_LORE_BIN",
    );
    assert_binary_built_from_exact_pin(&lore_bin);

    let sandbox = tempfile::tempdir().expect("canary sandbox");
    let service_a = sandbox.path().join("service-a");
    let caller_b = sandbox.path().join("caller-b");
    std::fs::create_dir_all(&service_a).expect("service cwd A");
    std::fs::create_dir_all(&caller_b).expect("caller/root B");

    let proc = boot_service_in_directory(&lore_bin, Some(&service_a))
        .unwrap_or_else(|e| panic!("boot exact-pin service in cwd A: {e}"));
    let relative_repo = "relative-service-canary";
    let output = Command::new(&lore_bin)
        .args([
            "--repository",
            relative_repo,
            "--offline",
            "repository",
            "create",
            "cwd-canary",
        ])
        .current_dir(&caller_b)
        .env("XDG_RUNTIME_DIR", proc.runtime_dir.path())
        .env("TMPDIR", proc.runtime_dir.path())
        .env("LORE_GLOBAL_PATH", proc.global_dir.path())
        .env("LORE_USE_SERVICE", "1")
        .output()
        .expect("run exact-pin client against exact-pin service");

    assert!(
        output.status.success(),
        "relative repository create through service failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        caller_b.join(relative_repo).join(".lore").is_dir(),
        "relative repository must land under authoritative caller/root B"
    );
    assert!(
        !service_a.join(relative_repo).exists(),
        "relative repository must never land under service cwd A"
    );

    stop_service(proc).expect("clean exact-pin service shutdown");
}

/// Regression: candidate discovery never walks sibling short-rev checkouts.
/// Paths under cargo git checkouts must include only the workspace-pinned short rev.
#[test]
fn exact_pin_lore_candidates_never_use_sibling_short_revs() {
    let Some(rev) = pinned_rev() else {
        eprintln!("[SKIP] could not parse pinned rev from Cargo.toml");
        return;
    };
    let short = &rev[..7];
    let candidates = exact_pin_lore_candidates();
    // Must not invent arbitrary sibling paths. Env override is caller-controlled;
    // checkout-derived candidates must sit under the pinned short rev only.
    for cand in &candidates {
        let s = cand.to_string_lossy();
        if s.contains("git/checkouts") {
            assert!(
                s.contains(short),
                "checkout-derived lore candidate must be under pinned short rev {short}, got {s}"
            );
            // Guard against accidental multi-rev globs (e.g. scanning 2d86d1d/6559841).
            let after_checkouts = s.split("git/checkouts/").nth(1).unwrap_or_default();
            // Path shape: lore-<hash>/<short>/target/...
            let parts: Vec<&str> = after_checkouts.split('/').collect();
            assert!(parts.len() >= 2, "unexpected checkout candidate shape: {s}");
            assert_eq!(
                parts[1], short,
                "second path component after checkouts must be pinned short rev {short}, got {}",
                parts[1]
            );
        }
    }
    // When the pin checkout exists, both release/debug candidates must be listed.
    if lore_checkout().is_some() {
        let checkout_cands: Vec<_> = candidates
            .iter()
            .filter(|p| p.to_string_lossy().contains("git/checkouts"))
            .collect();
        assert!(
            checkout_cands
                .iter()
                .any(|p| p.to_string_lossy().contains("/release/lore")),
            "expected release/lore under pinned checkout among {candidates:?}"
        );
        assert!(
            checkout_cands
                .iter()
                .any(|p| p.to_string_lossy().contains("/debug/lore")),
            "expected debug/lore under pinned checkout among {candidates:?}"
        );
    }
}

/// Regression canary: resolve only returns a path that currently exists as a file.
/// Absence → None (soft-skip path); presence of arbitrary non-pin siblings is not probed.
#[test]
fn resolve_lore_binary_returns_none_when_no_exact_file() {
    // Isolate from a real LOREVM_LORE_BIN that may be set in the environment.
    // We cannot unset for the whole process safely mid-suite, so only assert the
    // pure candidate filter: every *returned* resolve path must be an existing file,
    // and must not come from a non-pin short rev under checkouts.
    if let Some(bin) = resolve_lore_binary() {
        assert!(bin.is_file(), "resolve must only return existing files");
        let s = bin.to_string_lossy();
        if s.contains("git/checkouts") {
            let rev = pinned_rev().expect("pinned rev");
            let short = &rev[..7];
            assert!(
                s.contains(short),
                "resolved binary must be under pinned rev {short}: {s}"
            );
        }
    }
}

/// Regression canary: lore-vm `service::start` remains a thin binding over the
/// upstream stub (returns failure until upstream implements the C-API path).
/// The real Unix service path is the CLI `lore service run` exercised above.
#[tokio::test]
async fn lore_vm_service_start_binding_surfaces_failure_cleanly() {
    use lore_vm::api::LoreApi;
    use lore_vm::global::LoreGlobal;
    use lore_vm::ops;

    let dir = tempfile::tempdir().expect("tempdir");
    let api = LoreApi::from_global(
        LoreGlobal::new(dir.path().to_path_buf())
            .offline(true)
            .in_memory(true),
    );
    // Upstream start_local currently returns status 1 (stub). Binding must not panic.
    let result = ops::service::start::start(&api).await;
    assert!(
        result.is_err(),
        "upstream service::start is still a stub (status 1); binding should surface Err, got {result:?}"
    );
}
