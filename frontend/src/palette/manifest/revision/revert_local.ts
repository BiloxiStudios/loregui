import type { OpManifest } from "../../types";

/**
 * Manifest entry for `revision.revert_local`.
 *
 * Reverts the working directory to a specified revision by applying the
 * inverse of its changes. Supports optional auto-commit when no conflicts
 * arise.
 */
const manifest: OpManifest = {
  id: "revision.revert_local",
  domain: "revision",
  op: "revert_local",
  label: "Revision: Revert Local",
  description:
    "Revert the working directory to a specified revision by applying the inverse of its changes.",
  command: "revision_revert_local",
  args: [
    {
      name: "revision",
      kind: "text",
      label: "Revision",
      required: true,
      placeholder: "abc123",
    },
    {
      name: "message",
      kind: "text",
      label: "Message",
      description: "Commit message for the auto-commit when no conflicts arise.",
      placeholder: "Revert to revision",
    },
    {
      name: "noCommit",
      kind: "boolean",
      label: "No Auto-Commit",
      description: "Skip auto-commit even if there are no conflicts.",
    },
  ],
  resultKind: "json",
  keywords: ["revert", "undo", "rollback"],
};

export default manifest;
