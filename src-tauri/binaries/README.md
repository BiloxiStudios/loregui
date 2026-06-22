# Tauri sidecar binaries (`externalBin`) — SBAI-4069

This directory holds the **`loreserver`** sidecar that LoreGUI's "Host a server"
flow spawns in shipped builds. `tauri.conf.json` declares it via
`bundle.externalBin: ["binaries/loreserver"]`.

## Naming (Tauri `externalBin` contract)

Tauri resolves an `externalBin` entry at **build time** by appending the build's
**target triple** (and `.exe` on Windows) to the base name. So for the entry
`binaries/loreserver`, the bundler looks for a file named:

| Platform (CI runner)        | Rust target triple             | Staged file name                                   |
| --------------------------- | ------------------------------ | -------------------------------------------------- |
| Linux x86_64 (`ubuntu`)     | `x86_64-unknown-linux-gnu`     | `loreserver-x86_64-unknown-linux-gnu`              |
| Windows x86_64 (`windows`)  | `x86_64-pc-windows-msvc`       | `loreserver-x86_64-pc-windows-msvc.exe`            |
| macOS Apple Silicon (`macos`)| `aarch64-apple-darwin`        | `loreserver-aarch64-apple-darwin`                  |

At **runtime** Tauri ships the matched binary next to the app executable under
the **bare** name (`loreserver` / `loreserver.exe`); LoreGUI's
`server_host::resolve_server_binary` looks for it there first (the production
path).

## Why this directory is (mostly) empty in git

The `loreserver` binary is a heavy (~1 GB debug / large release) artifact built
from the pinned upstream `lore` checkout
(`cargo build -p lore-server --bin loreserver`). It is **never committed**. CI's
release workflow (`.github/workflows/release.yml`) builds it for each matrix
target and stages it here as `loreserver-<target-triple>[.exe]` *before* running
`tauri build`. `.gitignore` ignores everything here except this README.

## Dev / override

Local development does not need a staged sidecar: `src-tauri/build.rs`
auto-writes a stub placeholder here for the current target triple so the Tauri
build script is satisfied during `cargo check` / `cargo test` / `tauri dev`. To
run a **real** hosted server in dev, set `LOREVM_SERVER_BIN` to a locally-built
`loreserver` (which overrides), or let the dev-checkout fallback build it from
the pinned `lore` rev. See `docs/host-server-sidecar.md`.
