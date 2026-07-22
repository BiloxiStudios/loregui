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
  requiresRepository: false,
  args: [
    {
      name: "installAutorun",
      kind: "boolean",
      label: "Install autorun",
      description:
        "Register the service to start automatically. The op is a deprecated stub today; the Rust command requires this flag to be present.",
      required: false,
      default: false,
    },
  ],
  resultKind: "void",
  keywords: ["start", "service", "daemon", "run"],
};

export default manifest;
