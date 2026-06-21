import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for branch.latest_list.
 *
 * Lists the LATEST revision history for a branch, returning one entry per
 * revision pointer in the branch's latest-chain. Each entry carries the
 * branch identifier and the revision hash.
 */
const manifest: OpManifest = {
  id: "branch.latest_list",
  domain: "branch",
  op: "latest_list",
  label: "Branch: Latest List",
  description:
    "List the latest revision history for a branch (newest first).",
  command: "branch_latest_list",
  args: [
    {
      name: "branch",
      kind: "text",
      label: "Branch",
      description: "Branch name; empty for current branch.",
      required: false,
      placeholder: "e.g. main",
    },
    {
      name: "limit",
      kind: "number",
      label: "Limit",
      description:
        "Maximum entries to return; 0 uses the default of 30.",
      required: false,
      default: 0,
    },
  ],
  resultKind: "json",
  keywords: ["latest", "branch", "history", "revisions", "chain"],
};

export default manifest;
