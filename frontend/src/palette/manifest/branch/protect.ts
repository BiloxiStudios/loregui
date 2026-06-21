import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for branch.protect.
 *
 * Applies write protection to a branch, preventing direct commits.
 * Protected branches require merge requests for changes.
 */
const manifest: OpManifest = {
  id: "branch.protect",
  domain: "branch",
  op: "protect",
  label: "Branch: Protect",
  description:
    "Apply write protection to a branch, preventing direct commits.",
  command: "branch_protect",
  args: [
    {
      name: "branch",
      kind: "text",
      label: "Branch",
      description: "Name of the branch to protect.",
      required: true,
      placeholder: "e.g. main",
    },
  ],
  resultKind: "json",
  keywords: ["branch", "protect", "lock", "write", "guard", "permission"],
};

export default manifest;
