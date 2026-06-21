import type { OpManifest } from "../../types";

/** Restart conflict resolution for the given paths in a branch merge. */
const manifest: OpManifest = {
  id: "branch.merge_restart",
  domain: "branch",
  op: "merge_restart",
  label: "Branch: Restart Merge Resolution",
  description:
    "Restart conflict resolution for the given paths in an in-progress branch merge.",
  command: "branch_merge_restart",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description: "Paths to restart merge resolution for (one per line).",
      required: true,
    },
  ],
  resultKind: "json",
  keywords: ["branch", "merge", "restart", "resolve", "conflict", "redo"],
};

export default manifest;
