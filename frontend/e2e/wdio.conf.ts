// WebdriverIO config for the LoreGUI Tauri v2 desktop smoke suite.
//
// Drives the BUILT desktop binary through `tauri-driver`, which bridges WDIO's
// WebDriver protocol to the platform WebView (WebKitWebGTK / WKWebView / Web
// View2). On Linux this runs headless under `xvfb-run` — see README.md and
// `.github/workflows/tauri-e2e.yml`.
//
// tauri-driver spawns the app for each session; we only point it at the
// binary and let it manage the native WebDriver (WebKitWebDriver on Linux).
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import os from "node:os";

// --- locate the built LoreGUI binary --------------------------------------
// Built by `cargo tauri build` (debug or release). The product binary is
// `loregui` (see src-tauri/Cargo.toml `name`). Allow an explicit override so
// CI / local devs can point at any build output.
const repoRoot = resolve(__dirname, "..", "..");
function firstExisting(paths: string[]): string | undefined {
  return paths.find((p) => existsSync(p));
}
const platformBin =
  process.platform === "win32" ? "loregui.exe" : "loregui";
const application =
  process.env.LOREGUI_E2E_BINARY ??
  firstExisting([
    resolve(repoRoot, "target", "release", platformBin),
    resolve(repoRoot, "target", "debug", platformBin),
  ]) ??
  resolve(repoRoot, "target", "release", platformBin);

// tauri-driver process handle (started in onPrepare, killed in onComplete).
let tauriDriver: ChildProcess | undefined;

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./specs/**/*.e2e.ts"],
  maxInstances: 1,
  capabilities: [
    {
      // `tauri:options` is consumed by tauri-driver to launch the app.
      "tauri:options": {
        application,
      } as Record<string, unknown>,
      // tauri-driver proxies to the native WebDriver; WDIO still needs a
      // browserName to route the session.
      browserName: "wry",
    } as WebdriverIO.Capabilities,
  ],
  // tauri-driver listens here by default.
  hostname: "127.0.0.1",
  port: 4444,
  path: "/",
  reporters: ["spec"],
  framework: "mocha",
  mochaOpts: {
    ui: "bdd",
    // The first session has to boot the whole native app + WebView; be patient.
    timeout: 120_000,
  },
  logLevel: "warn",
  // Fail fast if the binary is missing — a clearer message than a cryptic
  // session-create error from tauri-driver.
  onPrepare: () => {
    if (!existsSync(application)) {
      throw new Error(
        `LoreGUI binary not found at ${application}.\n` +
          `Build it first (cargo tauri build, or the debug build) or set ` +
          `LOREGUI_E2E_BINARY. See frontend/e2e/README.md.`,
      );
    }
    // `tauri-driver` must be installed (cargo install tauri-driver). It in turn
    // needs the platform WebDriver (Linux: WebKitWebDriver from
    // webkit2gtk-driver). Both are installed in the CI workflow.
    const driverBin = process.env.TAURI_DRIVER_BIN ?? "tauri-driver";
    tauriDriver = spawn(driverBin, [], {
      stdio: [null, process.stdout, process.stderr],
    });
    tauriDriver.on("error", (err) => {
      // eslint-disable-next-line no-console
      console.error(
        `Failed to launch ${driverBin}. Install with \`cargo install tauri-driver --locked\`.`,
        err,
      );
    });
  },
  onComplete: () => {
    tauriDriver?.kill();
  },
};

// Surface unused import lint cleanliness for the few node builtins we keep
// around for local debugging convenience.
void spawnSync;
void os;
