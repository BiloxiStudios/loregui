# LoreGUI Tauri E2E (WebDriver smoke suite)

Two layers of end-to-end coverage land in this PR. **Pick the layer that fits
your loop:**

| Layer | Where | What it drives | Cost | Run |
|-------|-------|----------------|------|-----|
| **WebDriver smoke** (this dir) | `frontend/e2e/` | the **built desktop binary** + the **real WebView**, through `tauri-driver` | heavy (full `tauri build` + native WebDriver + a display) | `npm --prefix frontend/e2e test` |
| **IPC harness** | `src-tauri/src/ipc_harness_tests.rs` | the `#[tauri::command]` layer in-process via Tauri `MockRuntime` — no WebView, no binary | light (`cargo test`) | `cargo test -p loregui` |

The IPC harness is the fast gate (runs anywhere, including CI without a
display); the WebDriver suite is the real "does the app actually boot and
render and round-trip IPC" gate. CI runs both — see
`.github/workflows/tauri-e2e.yml`.

## What the WebDriver suite checks (breadth-first)

`specs/smoke.e2e.ts`:

1. the app **boots** and the main window **renders** (`.app` shell mounts),
2. **onboarding is skippable** via the same `localStorage["loregui.onboarded"]`
   gate the app uses,
3. the **topbar** and the key **panels mount** — Branches, History,
   Repository/Manage, Storage, Account — and the **Status** (changes) view
   renders,
4. a clean-profile, explicit non-repository process CWD boots into the project
   hub, with repository actions disabled and a positive-controlled IPC audit
   proving those clicks emit no commands,
5. core **IPC round-trips**:
   - a read-only pair (`current_repository`, `auth_local_user_info`),
   - the VCS **read** commands (`status` / `log` / `branches`) against the real
     in-process lore engine — deterministic, no network,
   - an unreachable `repository_create` must propagate the exact typed failure
     and leave `current_repository` null — never a green skip,
   - the deterministic fixture-owned host-store → local-repository open → app
     state rebuild/restoration journey lives in the IPC harness. It uses
     separate server-store and client-working-tree paths and validates stale
     persisted paths fail closed.

## How it works

- **`tauri-driver`** bridges WebdriverIO's WebDriver protocol to the platform
  WebView. On Linux it proxies to **`WebKitWebDriver`** (from the
  `webkit2gtk-driver` package). It launches the app binary for each session.
- The suite reaches the Rust command layer from the page via
  `window.__TAURI__.core.invoke`. That global is only present because the E2E
  build is made with **`src-tauri/tauri.e2e.conf.json`**, a config overlay whose
  *only* change is `withGlobalTauri: true`. **The shipped binary keeps it OFF.**
- `wdio.conf.ts` starts `tauri-driver` in a fixture-owned non-repository current
  directory and, on Linux, supplies a fixture-owned `XDG_CONFIG_HOME`. This
  prevents a runner/developer `settings.json` or checkout CWD from becoming
  accidental repository context.
- The IPC no-action audit arms an explicit DOM marker and calls the same
  centralized `api.ts` invoke wrapper used by guarded UI actions. That wrapper
  records **command names only** and only when both the marker and the E2E
  overlay's `window.__TAURI__` global exist. The shipped config has
  `withGlobalTauri: false`; unit negatives prove the marker alone cannot enable
  auditing and arguments/tokens never enter the log.

## Run it locally (Linux)

```sh
# 1. one-time tooling
cargo install tauri-driver --locked
sudo apt-get install -y webkit2gtk-driver xvfb        # native WebDriver + headless X

# 2. install deps
npm --prefix frontend ci
npm --prefix frontend/e2e ci

# 3. build the app WITH the E2E config overlay (exposes window.__TAURI__)
cargo tauri build --debug --config src-tauri/tauri.e2e.conf.json
#   (or `cargo tauri build --config …` for a release build)

# 4. run the suite headless
xvfb-run -a npm --prefix frontend/e2e test
```

### Knobs

- `LOREGUI_E2E_BINARY` — absolute path to the built binary (skips auto-detect of
  `target/{release,debug}/loregui`).
- `TAURI_DRIVER_BIN` — path to `tauri-driver` if not on `PATH`.

## CI

`.github/workflows/tauri-e2e.yml` runs **both** layers on Linux:
the `cargo test -p loregui` IPC harness, and the WebDriver smoke suite under
`xvfb` against a debug build made with the E2E config overlay. It is a separate
workflow from `ci.yml` (kept fast) and `windows-build.yml`.

## macOS / Windows

The same suite runs on macOS (`WKWebView` via the system `WebDriver`) and
Windows (`Microsoft.WebView2` via `msedgedriver`). The CI workflow here targets
Linux only for now; the config is cross-platform (`platformBin` switches the
binary name) so it can be extended.
