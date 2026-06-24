// In-app auto-update (SBAI-4040).
//
// Minimal check → prompt → download+install → relaunch flow on top of
// `@tauri-apps/plugin-updater`. The update artifacts and the `latest.json`
// manifest are produced + published by `.github/workflows/release.yml` and
// signed with the updater private key (GH secret TAURI_SIGNING_PRIVATE_KEY).
// The matching public key lives in `src-tauri/tauri.conf.json` →
// `plugins.updater.pubkey`, so the client refuses any unsigned/tampered update.
//
// This is intentionally tiny and dependency-free (uses the browser `confirm`
// dialog) so it works in the open core without pulling in extra UI. A richer
// in-app "update available" surface can replace `confirm()` later.
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

/** True only inside the Tauri webview (the updater is a no-op in browser dev). */
function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * Check for an update and, if the user accepts, download + install it and
 * relaunch. Safe to call unconditionally: it silently no-ops outside Tauri and
 * never throws into the caller (failures are logged, not surfaced).
 *
 * @param prompt - confirmation gate; defaults to the browser `confirm` dialog.
 *                 Pass a custom async prompt to drive a themed in-app modal.
 * @returns `true` if an install+relaunch was initiated, `false` otherwise.
 */
export async function checkForUpdates(
  prompt: (version: string, notes: string) => boolean | Promise<boolean> = (
    version,
    notes,
  ) =>
    window.confirm(
      `LoreGUI ${version} is available.\n\n${notes || ""}\n\nDownload and install now? The app will restart.`,
    ),
): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    const update = await check();
    if (!update) return false; // already up to date

    const accepted = await prompt(update.version, update.body ?? "");
    if (!accepted) return false;

    // Streams the (signature-verified) artifact and applies it. On Windows the
    // configured `passive` installMode runs the NSIS/MSI installer with a
    // minimal UI; on macOS/Linux the bundle is swapped in place.
    await update.downloadAndInstall();

    // Replace the now-stale running process with the freshly installed binary.
    await relaunch();
    return true;
  } catch (err) {
    // Offline, no release yet, signature mismatch, etc. — never block startup.
    console.warn("[updater] update check failed:", err);
    return false;
  }
}
