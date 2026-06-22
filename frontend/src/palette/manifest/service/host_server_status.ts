import type { OpManifest } from "../../types";

/**
 * Command-palette manifest entry for `host server: status` (SBAI-4065).
 *
 * Reports whether a hosted `loreserver` is running and, if so, its pid, port,
 * and the `lore://host:port/<repo>` URL clients connect to. Reaps the process if
 * it has exited so the status reflects reality.
 */
const manifest: OpManifest = {
  id: "service.host_server_status",
  domain: "service",
  op: "host_server_status",
  label: "Host Server: Status",
  description:
    "Show whether a hosted Lore server is running, with its pid, port, and lore:// URL.",
  command: "host_server_status",
  args: [],
  resultKind: "json",
  keywords: ["host", "server", "loreserver", "status", "running", "url"],
};

export default manifest;
