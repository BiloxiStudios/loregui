import type { OpManifest } from "../../types";

/**
 * Reference manifest entry (Phase 0). A no-arg op whose result is a typed
 * object — exercises the empty-form + JSON-result path of the palette.
 */
const manifest: OpManifest = {
  id: "repository.instance_prune",
  domain: "repository",
  op: "instance_prune",
  label: "Repository: Prune Stale Instances",
  description:
    "Remove stale repository instances (working copies whose filesystem paths no longer exist) from the shared store.",
  command: "repository_instance_prune",
  args: [],
  resultKind: "json",
  keywords: ["prune", "cleanup", "stale", "instances", "garbage"],
};

export default manifest;
