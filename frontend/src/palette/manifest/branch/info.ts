import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for branch.info.
 *
 * Retrieves metadata for a branch including its name, id, category,
 * protection status, parent, creation time, and archive state.
 */
const manifest: OpManifest = {
  id: "branch.info",
  domain: "branch",
  op: "info",
  label: "Branch: Info",
  description:
    "Show metadata for a branch — name, category, parent, creator, timestamps, and archive state.",
  command: "branch_info",
  args: [
    {
      name: "branch",
      kind: "text",
      label: "Branch",
      description: "Branch name; leave empty for the current branch.",
      required: false,
      placeholder: "e.g. main",
    },
  ],
  resultKind: "json",
  keywords: ["branch", "info", "metadata", "details", "status"],
};

export default manifest;
