import type { OpManifest } from "../../types";

/**
 * Command-palette manifest entry for `service stop`.
 *
 * Stops the Lore service process for the current repository (or all repositories
 * when `all` is set). Returns diagnostic log messages from the stop operation.
 */
const manifest: OpManifest = {
  id: "service.stop",
  domain: "service",
  op: "stop",
  label: "Service: Stop",
  description:
    "Stop the Lore service process for the current repository (or all repositories).",
  command: "service_stop",
  args: [
    {
      name: "all",
      kind: "boolean",
      label: "All repositories",
      description:
        "Stop the service for all repositories rather than just the current one.",
      required: false,
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["stop", "service", "daemon", "halt", "shutdown"],
};

export default manifest;
