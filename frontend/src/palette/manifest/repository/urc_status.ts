import type { OpManifest } from "../../types";

/**
 * Palette manifest for `repository.urc_status` (SBAI-5499).
 *
 * Local working-tree (URC) health snapshot: current/remote revisions, pending
 * merge, divergence, staged paths, conflicts. Also rendered first-class in the
 * Changes panel (UrcStatusCard), hence `surface: "panel"`.
 */
const manifest: OpManifest = {
  id: "repository.urc_status",
  domain: "repository",
  op: "urc_status",
  label: "Repository: URC Status",
  description:
    "Show local working-tree health — current/remote revision, pending merge, divergence, staged paths, conflicts.",
  command: "repository_urc_status",
  args: [],
  resultKind: "json",
  surface: "panel",
  keywords: [
    "status",
    "urc",
    "working tree",
    "health",
    "conflicts",
    "diverged",
    "merge",
  ],
};

export default manifest;
