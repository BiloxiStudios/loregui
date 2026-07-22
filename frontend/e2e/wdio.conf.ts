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
import { existsSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import os from "node:os";

// This package is ESM (`"type": "module"`), so `__dirname` is not defined —
// derive it from `import.meta.url`.
const __dirname = dirname(fileURLToPath(import.meta.url));

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
let isolatedProcessCwd: string | undefined;

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./specs/**/*.e2e.ts"],
  maxInstances: 1,
  capabilities: [
    {
      // `tauri:options` is consumed by tauri-driver to launch the app. On Linux
      // tauri-driver rewrites it into `webkitgtk:browserOptions` and forwards to
      // WebKitWebDriver, which matches on those options — so we must NOT set a
      // `browserName` here (an unknown one like "wry" makes WebKitWebDriver
      // reject the session with "Failed to match capabilities").
      "tauri:options": {
        application,
      } as Record<string, unknown>,
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
    // Reproduce the Windows-autostart class of bug under a deterministic,
    // explicit directory that is NOT a repository. The app must derive active
    // repository state only from validated persisted/selected paths, never the
    // driver/app process CWD.
    isolatedProcessCwd = mkdtempSync(join(os.tmpdir(), "loregui-e2e-nonrepo-cwd-"));
    const isolatedConfigHome = join(isolatedProcessCwd, "xdg-config");
    mkdirSync(isolatedConfigHome, { recursive: true });
    tauriDriver = spawn(driverBin, [], {
      stdio: [null, process.stdout, process.stderr],
      cwd: isolatedProcessCwd,
      // Linux's app_config_dir resolves through XDG_CONFIG_HOME. Isolating it
      // proves startup cannot consume a developer/runner LoreGUI settings.json
      // while the positive current_repository=null assertion proves the child
      // actually launched with a clean profile.
      env: {
        ...process.env,
        ...(process.platform === "linux"
          ? { XDG_CONFIG_HOME: isolatedConfigHome }
          : {}),
      },
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
    if (isolatedProcessCwd) {
      rmSync(isolatedProcessCwd, { recursive: true, force: true });
    }
  },
};

// Surface unused import lint cleanliness for the few node builtins we keep
// around for local debugging convenience.
void spawnSync;
void os;
