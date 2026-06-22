import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { ThemeProvider } from "./theme/ThemeProvider";
import { bootstrapEntitlements } from "./commercial/entitlement";
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
        <App />
      </ThemeProvider>
    </React.StrictMode>,
  );
}

// Resolve + verify the offline signed license (SBAI-4068) BEFORE React mounts so
// `isEntitled()` reflects it synchronously at every call site. A missing/invalid
// license is a no-op — the open core mounts identically and stays fully working.
void bootstrapEntitlements(loadLicenseFile).finally(mount);
