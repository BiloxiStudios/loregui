# LoreGUI desktop E2E (WebDriver / tauri-driver)

First end-to-end coverage for the packaged desktop app. The suite launches the
**real built `loregui` binary** through [`tauri-driver`](https://v2.tauri.app/develop/tests/webdriver/)
and drives the genuine WebKitGTK webview + Rust IPC — no stubs, no mocks.

## What it covers (breadth-first smoke)

`specs/smoke.spec.ts`:

1. App boots and the webview document renders.
2. The global Tauri IPC bridge (`window.__TAURI__.core.invoke`) is present.
3. The main view mounts after onboarding is satisfied (top-bar brand + nav).
4. Core nav panels mount: **Branches / History / Locks / Manage**.
5. Core IPC round-trips: `open_repository` → `current_repository`.
6. Engine IPC against a fresh (non-repo) dir resolves promptly without hanging
   the bridge (`status` / `branches` / `log`).
7. The command palette opens (⌘K).

## Two layers of desktop coverage

| Layer | Where | Needs a display? | What it gates |
|-------|-------|------------------|---------------|
| **IPC unit** | `src-tauri/src/ipc_smoke.rs` (`cargo test -p loregui`) | No (MockRuntime) | Command registration, state plumbing, arg/result serde, the async error path doesn't hang |
| **Full E2E** | this dir (`tauri-driver` + WebKitWebDriver) | Yes (xvfb) | The real binary boots, webview renders, panels mount, IPC round-trips |

The fast MockRuntime gate runs everywhere (CI `core-check`-style). The heavy E2E
runs in `tauri-e2e.yml`.

## Prerequisites (Linux)

```sh
# WebKitGTK webview + its WebDriver
sudo apt-get install -y libwebkit2gtk-4.1-dev webkit2gtk-driver xvfb \
    libgtk-3-dev librsvg2-dev patchelf libayatana-appindicator3-dev

# The WebDriver proxy Tauri ships
cargo install tauri-driver --locked
```

## Build the app for E2E

The E2E build enables `withGlobalTauri` via a config overlay so the suite can
reach `invoke` from inside the webview. The shipped production config is NOT
modified.

```sh
npm --prefix frontend ci
npm --prefix frontend run build
cargo tauri build --no-bundle --config e2e/tauri.e2e.conf.json
# → produces src-tauri/target/release/loregui
```

(A plain `cargo build --release -p loregui` also works for a debug-of-release run;
set `LOREGUI_E2E_BIN` to point the suite at any binary.)

## Run

```sh
npm --prefix e2e ci
xvfb-run -a npm --prefix e2e test
```

Override the binary location with `LOREGUI_E2E_BIN`, the proxy with
`TAURI_DRIVER_BIN`.

## Notes / gotchas

- **Headless via xvfb.** tauri-driver → WebKitWebDriver needs an X display;
  `xvfb-run -a` provides a virtual one. There is no native headless WebKitGTK.
- The suite bypasses the onboarding wizard by setting
  `localStorage["loregui.onboarded"] = "true"` and reloading — the same hook the
  visual harness uses.
- The placeholder `loreserver` sidecar (auto-staged by `src-tauri/build.rs`) is
  enough; the smoke suite never hosts a real server.
