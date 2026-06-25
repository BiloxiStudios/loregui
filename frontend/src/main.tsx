import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import ErrorBoundary from "./ErrorBoundary";
import { ThemeProvider } from "./theme/ThemeProvider";
import { bootstrapEntitlements } from "./commercial/entitlement";
// Commercial overlay entry (SBAI-4061 / SBAI-4068): imported once for its side
// effects so any premium overlay can register its panels into the premium
// registry before React mounts. In the open core this is an EMPTY stub that
// registers nothing; a commercial build swaps it for the loregui-cloud overlay
// entry. Keep this import even in open core — it is the seam.
import "./commercial/overlay-entry";
import { checkForUpdates } from "./update";
import "./styles.css";

/** Read an on-disk `license.key` via the Tauri command; null outside Tauri. */
async function loadLicenseFile(): Promise<string | null> {
  try {
    return (await invoke<string | null>("read_license_file")) ?? null;
  } catch {
    // Not running under Tauri (e.g. browser dev) or command unavailable.
    return null;
  }
}

function mount(): void {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <ThemeProvider>
        <ErrorBoundary>
          <App />
        </ErrorBoundary>
      </ThemeProvider>
    </React.StrictMode>,
  );
}

// Resolve + verify the offline signed license (SBAI-4068) BEFORE React mounts so
// `isEntitled()` reflects it synchronously at every call site. A missing/invalid
// license is a no-op — the open core mounts identically and stays fully working.
void bootstrapEntitlements(loadLicenseFile).finally(() => {
  mount();
  // Fire-and-forget auto-update check (SBAI-4040) after the UI is up. No-ops
  // outside Tauri and never throws; prompts the user before installing.
  void checkForUpdates();
});
