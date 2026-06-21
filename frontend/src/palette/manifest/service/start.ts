import type { OpManifest } from "../../types";

/**
 * Command-palette manifest entry for `service start`.
 *
 * Starts the Lore service process for the current repository. The upstream
 * lore-vm op takes no arguments; the Tauri command accepts an `installAutorun`
 * flag for forward compatibility but does not yet act on it.
 */
const manifest: OpManifest = {
  id: "service.start",
  domain: "service",
  op: "start",
  label: "Service: Start",
  description: "Start the Lore service process for the current repository.",
  command: "service_start",
  args: [],
  resultKind: "void",
  keywords: ["start", "service", "daemon", "run"],
};

export default manifest;
