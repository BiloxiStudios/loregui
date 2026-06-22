# Hosting a server in shipped builds — the `loreserver` sidecar (SBAI-4069)

LoreGUI's **"Host a server"** flow (SBAI-4065, `src-tauri/src/server_host.rs`)
spawns the real upstream **`loreserver`** binary (crate `lore-server`, bin
`loreserver`). For this to work in a **packaged installer** — which has no
`LOREVM_SERVER_BIN` env var and no pinned `lore` dev checkout to build from — the
server is shipped as a **Tauri `externalBin` sidecar**.

## How `loreserver` is resolved at runtime

`server_host::resolve_server_binary` tries these sources **in order**:

1. **Bundled sidecar (production path).** The `loreserver` / `loreserver.exe`
   that Tauri ships next to the app executable. This is checked **first**, so a
   shipped build never needs an env var or a dev checkout.
2. **`LOREVM_SERVER_BIN` (dev / override).** Point this at a locally-built
   `loreserver` and it takes over when no sidecar is bundled (the normal dev
   case). A value that is set but doesn't point at a file is a **hard error**
   (we don't silently ignore an explicit override). The SBAI-4064 spike used
   this.
3. **Dev-checkout build fallback.** Builds `loreserver` from the pinned upstream
   `lore` git checkout (`cargo build -p lore-server --bin loreserver`) — slow,
   dev-only, and never reached in a release build because the sidecar resolves
   at step 1.

The resolution order is unit-tested (`server_host::tests::resolution_*`) via the
pure `resolve_production_binary` helper, so the sidecar-first priority can't
regress.

> **Note:** the bundled sidecar wins even when `LOREVM_SERVER_BIN` is also set.
> To force a custom binary in a shipped build, you'd remove/replace the bundled
> sidecar — but `LOREVM_SERVER_BIN` remains the supported override for **dev**,
> where no sidecar is bundled.

## How the sidecar is bundled

`src-tauri/tauri.conf.json`:

```jsonc
"bundle": {
  "externalBin": ["binaries/loreserver"]
}
```

Tauri resolves an `externalBin` entry **at build time** by appending the build's
**target triple** (and `.exe` on Windows) to the base name, so the file the
bundler picks up is `src-tauri/binaries/loreserver-<target-triple>[.exe]`:

| CI runner / platform          | Rust target triple           | Staged file name                          |
| ----------------------------- | ---------------------------- | ----------------------------------------- |
| Linux x86_64 (`ubuntu-22.04`) | `x86_64-unknown-linux-gnu`   | `loreserver-x86_64-unknown-linux-gnu`     |
| Windows x86_64 (`windows`)    | `x86_64-pc-windows-msvc`     | `loreserver-x86_64-pc-windows-msvc.exe`   |
| macOS Apple Silicon (`macos`) | `aarch64-apple-darwin`       | `loreserver-aarch64-apple-darwin`         |

At runtime Tauri ships the matched binary next to the app under the **bare**
name, which is exactly where step 1 above looks.

The binaries are **not committed** (~1 GB build artifacts) — `.gitignore` ignores
everything under `src-tauri/binaries/` except its `README.md`.

## How CI stages the sidecar

`.github/workflows/release.yml` (the release matrix only — **not** the
`windows-build.yml` PR gate) builds the genuine `loreserver` for each target
**before** `tauri build`:

1. Read the pinned `lore` rev from `Cargo.toml` (used as the cache key).
2. `actions/cache` the staged binary, keyed on `triple + lore rev` — built once
   per rev per platform.
3. On a cache miss: `cargo fetch` to populate the cargo git cache, locate the
   unpacked lore checkout, then
   `cargo build --release -p lore-server --bin loreserver --target <triple>`.
   A `Swatinem/rust-cache` keyed on the same `triple + rev` keeps the heavy lore
   build incremental across runs.
4. Copy the result to `src-tauri/binaries/loreserver-<triple>[.exe]` and verify
   it exists before invoking `tauri build`.

The **PR gate** (`windows-build.yml`) deliberately does **not** run this ~1 GB
build. Because the `externalBin` declaration makes the Tauri build script require
*some* file at the staged path (even during plain `cargo check` / `cargo test` /
`tauri dev`), `src-tauri/build.rs` auto-stages a tiny **placeholder** for the
current target triple whenever no sidecar is present. The placeholder is a stub
that exits non-zero if ever spawned — it only satisfies the bundler's existence
check; it is git-ignored and never shipped. release.yml stages the real binary
*before* `tauri build`, and `build.rs` leaves an already-present sidecar intact,
so the genuine server is the one bundled in releases.

### Dev convenience: the `build.rs` placeholder

`cargo check`, `cargo test`, and `tauri dev` all run the Tauri build script,
which validates `externalBin` paths. So that developers don't have to manually
stage anything, `src-tauri/build.rs::ensure_sidecar_placeholder` writes a stub at
`src-tauri/binaries/loreserver-<current-triple>[.exe]` if one isn't there. To run
a **real** hosted server in dev, set `LOREVM_SERVER_BIN` (which overrides) or let
`server_host`'s dev-checkout fallback build the genuine `loreserver`.
