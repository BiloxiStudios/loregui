import type { OpManifest } from "../../types";

/**
 * Command-palette manifest entry for `host server: stop` (SBAI-4065).
 *
 * Stops the `loreserver` process launched by "Host Server: Start" (kill + reap).
 * Idempotent — a no-op if nothing is hosting. Returns the (now stopped) status.
 */
const manifest: OpManifest = {
  id: "service.host_server_stop",
  domain: "service",
  op: "host_server_stop",
  label: "Host Server: Stop",
  description: "Stop the hosted Lore server (loreserver) started from the GUI.",
  command: "host_server_stop",
  requiresRepository: false,
  args: [],
  resultKind: "json",
  keywords: ["host", "server", "loreserver", "stop", "halt", "shutdown"],
};

export default manifest;
