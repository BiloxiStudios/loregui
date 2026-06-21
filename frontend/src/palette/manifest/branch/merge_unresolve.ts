import type { OpManifest } from "../../types";

/**
 * Mark merge conflict files as unresolved. During a merge, conflicts can be
 * resolved by accepting one side or the other; this operation reverses that
 * resolution, returning the specified paths to an unresolved conflict state.
 */
const manifest: OpManifest = {
  id: "branch.merge_unresolve",
  domain: "branch",
  op: "merge_unresolve",
  label: "Branch: Unresolve Merge Conflicts",
  description: "Mark files as unresolved during a merge, reverting previous conflict resolutions.",
  command: "branch_merge_unresolve",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Files to unresolve",
      description: "Paths to mark as unresolved (empty = all conflicted files)",
      required: false,
      default: [],
    },
  ],
  resultKind: "json",
  keywords: ["merge", "conflict", "unresolve", "revert", "undo"],
};

export default manifest;
