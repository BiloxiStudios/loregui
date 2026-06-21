import type { OpManifest } from "../../types";

/**
 * Manifest entry for `revision.revert_resolve`.
 *
 * Marks the specified conflicted paths as resolved during an in-progress
 * revert. The user is expected to have manually edited the working-tree
 * copies before invoking this command.
 */
const manifest: OpManifest = {
  id: "revision.revert_resolve",
  domain: "revision",
  op: "revert_resolve",
  label: "Revision: Revert Resolve",
  description:
    "Mark conflicted paths as resolved during an in-progress revert.",
  command: "revision_revert_resolve",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      required: true,
      placeholder: "src/main.rs",
      description:
        "Repository-relative paths to mark as resolved.",
    },
  ],
  resultKind: "json",
  keywords: ["revert", "resolve", "conflict", "merge"],
};

export default manifest;
