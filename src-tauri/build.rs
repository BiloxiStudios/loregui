use std::path::PathBuf;

fn main() {
    ensure_sidecar_placeholder();
    tauri_build::build()
}

/// Ensure a `loreserver` sidecar exists for the current target triple so the
/// Tauri build script (which validates every `externalBin` path) succeeds during
/// plain `cargo check` / `cargo test` / `tauri dev`, where the real ~1 GB
/// upstream `loreserver` is not staged (SBAI-4069).
///
/// If a sidecar is already present (e.g. CI's release job staged the genuine
/// binary, or a developer dropped one in), it is left untouched. Otherwise a
/// tiny **placeholder** is written so the build proceeds. The placeholder is
/// NOT a working server — production resolution uses the real bundled sidecar
/// (release.yml), and dev hosting uses `LOREVM_SERVER_BIN` or the dev-checkout
/// build (see `src/server_host.rs`). The placeholder only satisfies the
/// bundler's existence check; it is git-ignored and never shipped by intent.
fn ensure_sidecar_placeholder() {
    // Tauri exports the resolved target triple to build scripts.
    let triple = match std::env::var("TARGET") {
        Ok(t) => t,
        Err(_) => return,
    };
    let exe_suffix = if triple.contains("windows") {
        ".exe"
    } else {
        ""
    };

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = manifest_dir.join("binaries");
    let sidecar = dir.join(format!("loreserver-{triple}{exe_suffix}"));

    println!("cargo:rerun-if-changed={}", sidecar.display());

    if sidecar.exists() {
        return;
    }
    if let Err(e) = std::fs::create_dir_all(&dir) {
        println!("cargo:warning=could not create {}: {e}", dir.display());
        return;
    }
    // A non-empty placeholder; make it executable on unix so a stray spawn fails
    // loudly rather than with a confusing permission error.
    if let Err(e) = std::fs::write(
        &sidecar,
        b"#!/bin/sh\necho 'loreserver placeholder (not a real server) - see docs/host-server-sidecar.md' >&2\nexit 1\n",
    ) {
        println!("cargo:warning=could not stage placeholder sidecar: {e}");
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&sidecar, std::fs::Permissions::from_mode(0o755));
    }
}
