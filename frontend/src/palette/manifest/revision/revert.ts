import type { OpManifest } from "../../types";

/**
 * Reference manifest entry (Phase 0). Reverts the working directory to a
 * specified revision. Exercises `text` (revision), `text` (message), and
 * `boolean` (noCommit) fields with a `json` result showing conflict status.
 */
const manifest: OpManifest = {
  id: "revision.revert",
  domain: "revision",
  op: "revert",
  label: "Revision: Revert",
  description:
    "Revert the working directory to a specified revision by applying the inverse of its changes.",
  command: "revision_revert_local",
  args: [
    {
      name: "revision",
      kind: "text",
      label: "Revision",
      required: true,
      placeholder: "abc123...",
    },
    {
      name: "message",
      kind: "text",
      label: "Message",
      description: "Commit message for the auto-commit when no conflicts arise.",
      placeholder: "Describe the revert",
    },
    {
      name: "noCommit",
      kind: "boolean",
      label: "No Commit",
      description: "Skip auto-commit even if there are no conflicts.",
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["revert", "undo", "rollback"],
};

export default manifest;
