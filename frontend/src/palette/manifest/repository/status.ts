import type { OpManifest } from "../../types";

/**
 * Reference manifest entry (Phase 0). A no-arg op whose result is a typed
 * object — exercises the empty-form + JSON-result path of the palette.
 */
const manifest: OpManifest = {
  id: "repository.status",
  domain: "repository",
  op: "status",
  label: "Repository: Status",
  description:
    "Show working-tree status — branch, revision, changes, ahead/behind.",
  command: "status",
  args: [],
  resultKind: "json",
  keywords: ["status", "changes", "dirty", "state"],
};

export default manifest;
