import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for revision.history.
 *
 * Retrieves the revision history for the current branch or a specified
 * revision. Returns a list of revision entries with hashes, numbers,
 * and parent references.
 */
const manifest: OpManifest = {
  id: "revision.history",
  domain: "revision",
  op: "history",
  label: "Revision: History",
  description: "Show revision history for the current branch or a specified revision.",
  command: "revision_history",
  args: [
    {
      name: "revision",
      kind: "text",
      label: "Revision",
      description: "Start from this revision; empty for current.",
      required: false,
      placeholder: "e.g. abc123def",
    },
    {
      name: "branch",
      kind: "text",
      label: "Branch",
      description: "Restrict to this branch; empty for current.",
      required: false,
      placeholder: "e.g. main",
    },
    {
      name: "date",
      kind: "number",
      label: "Date",
      description: "Stop at revisions created before this Unix timestamp; 0 disables.",
      required: false,
      default: 0,
    },
    {
      name: "length",
      kind: "number",
      label: "Length",
      description: "Maximum number of revisions to return; 0 for unlimited.",
      required: false,
      default: 0,
    },
    {
      name: "onlyBranch",
      kind: "boolean",
      label: "Only Branch",
      description: "Stop when reaching a different branch.",
      required: false,
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["history", "log", "revisions", "commits"],
};

export default manifest;
