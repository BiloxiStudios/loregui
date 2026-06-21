import type { OpManifest } from "../../types";

/**
 * Service start manifest entry. A no-arg op that starts the Lore service
 * process for the current repository.
 */
const manifest: OpManifest = {
  id: "service.start",
  domain: "service",
  op: "start",
  label: "Service: Start",
  description: "Start the Lore service process to manage the current repository.",
  command: "service_start",
  args: [],
  resultKind: "void",
  keywords: ["start", "service", "daemon", "run"],
};

export default manifest;
