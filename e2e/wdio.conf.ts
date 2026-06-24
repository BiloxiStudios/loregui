// WebdriverIO config for the LoreGUI desktop E2E smoke suite.
//
// Drives the REAL built Tauri binary through `tauri-driver`, which on Linux
// proxies WebDriver commands to `WebKitWebDriver` and launches the app's
// WebKitGTK webview. This is true end-to-end coverage: the actual Rust IPC
// layer, the actual React frontend, the actual command registration — no stubs.
//
// Requirements (installed by the workflow / DEVELOPING below):
//   - `tauri-driver`        (cargo install tauri-driver --locked)
//   - `WebKitWebDriver`     (apt: webkit2gtk-driver)
//   - the built binary at   ../src-tauri/target/release/loregui
//     (built with the e2e config overlay so withGlobalTauri is on)
//
// Run headless under xvfb:
//   xvfb-run -a npm --prefix e2e test
//
// Override the binary path with $LOREGUI_E2E_BIN if it lives elsewhere
// (e.g. a debug build, or a CI artifact path).

import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Resolve the application binary. Prefer an explicit override, then release,
// then debug — whichever the build produced.
function resolveAppBinary(): string {
  if (process.env.LOREGUI_E2E_BIN) return process.env.LOREGUI_E2E_BIN;
  const candidates = [
    resolve(__dirname, "../src-tauri/target/release/loregui"),
    resolve(__dirname, "../target/release/loregui"),
    resolve(__dirname, "../src-tauri/target/debug/loregui"),
    resolve(__dirname, "../target/debug/loregui"),
  ];
  const found = candidates.find((p) => existsSync(p));
  if (!found) {
    throw new Error(
      `Could not find the loregui binary. Looked in:\n  ${candidates.join(
        "\n  ",
      )}\nBuild it first (see e2e/README.md) or set $LOREGUI_E2E_BIN.`,
    );
  }
  return found;
}

const APP_BINARY = resolveAppBinary();
const TAURI_DRIVER =
  process.env.TAURI_DRIVER_BIN ||
  resolve(process.env.HOME || "", ".cargo/bin/tauri-driver");

let tauriDriver: ChildProcess | undefined;

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./specs/**/*.spec.ts"],
  maxInstances: 1,
  // tauri-driver listens on :4444 by default and speaks the W3C protocol.
  hostname: "127.0.0.1",
  port: 4444,
  capabilities: [
    {
      // `tauri:options.application` is the binary tauri-driver launches. On
      // Linux tauri-driver rewrites this into `webkitgtk:browserOptions` and
      // forwards to WebKitWebDriver — so we deliberately do NOT set a
      // `browserName` (WebKitWebDriver rejects an unknown one with
      // "Failed to match capabilities"; the webkitgtk options are the match).
      // @ts-expect-error tauri-specific capability not in the base WDIO types
      "tauri:options": { application: APP_BINARY },
    },
  ],
  framework: "mocha",
  mochaOpts: { ui: "bdd", timeout: 120_000 },
  reporters: ["spec"],
  logLevel: "warn",
  waitforTimeout: 15_000,
  connectionRetryTimeout: 120_000,
  // One retry only: tauri-driver is (re)spawned per session in beforeSession, so
  // a high retry count can collide on the proxy port (4444) if a prior attempt's
  // driver hasn't fully released it. afterSession kills it; one retry is plenty.
  connectionRetryCount: 1,

  // Spawn tauri-driver before the session, tear it down after.
  onPrepare: () => {
    if (!existsSync(TAURI_DRIVER)) {
      throw new Error(
        `tauri-driver not found at ${TAURI_DRIVER}. Install it with ` +
          `\`cargo install tauri-driver --locked\` or set $TAURI_DRIVER_BIN.`,
      );
    }
    // Sanity-check WebKitWebDriver is on PATH; tauri-driver shells out to it.
    const probe = spawnSync("WebKitWebDriver", ["--version"], {
      encoding: "utf8",
    });
    if (probe.error) {
      throw new Error(
        "WebKitWebDriver not found on PATH. Install the `webkit2gtk-driver` " +
          "package (Debian/Ubuntu) so tauri-driver can drive the webview.",
      );
    }
  },

  beforeSession: () =>
    new Promise<void>((res, rej) => {
      tauriDriver = spawn(TAURI_DRIVER, [], {
        stdio: [null, process.stdout, process.stderr],
      });
      tauriDriver.on("error", (e) => rej(e));
      // Give the proxy a moment to bind its port before the session connects.
      setTimeout(res, 1500);
    }),

  afterSession: () => {
    tauriDriver?.kill();
    tauriDriver = undefined;
  },
};
