import type { OpManifest } from "../../types";

/** List all branches in the repository, optionally including archived ones. */
const manifest: OpManifest = {
  id: "branch.list",
  domain: "branch",
  op: "list",
  label: "Branch: List",
  description:
    "List all branches in the repository, optionally including archived branches.",
  command: "branch_list",
  args: [
    {
      name: "archived",
      kind: "boolean",
      label: "Include archived",
      description: "When enabled, archived branches are included in the listing.",
      required: false,
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["branch", "list", "branches", "archived", "all"],
};

export default manifest;
