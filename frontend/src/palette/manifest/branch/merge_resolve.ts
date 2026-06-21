import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for branch.merge_resolve.
 *
 * Marks specified conflicted paths as resolved during an in-progress merge.
 * Returns the list of resolved paths and the updated staged revision.
 */
const manifest: OpManifest = {
  id: "branch.merge_resolve",
  domain: "branch",
  op: "merge_resolve",
  label: "Branch: Merge Resolve",
  description:
    "Mark conflicted files as resolved during an in-progress merge.",
  command: "branch_merge_resolve",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description: "Conflicted file paths to mark as resolved.",
      required: false,
      default: [],
    },
  ],
  resultKind: "json",
  keywords: ["branch", "merge", "resolve", "conflict", "paths"],
};

export default manifest;
