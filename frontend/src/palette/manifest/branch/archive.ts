import type { OpManifest } from "../../types";

/**
 * Reference manifest entry (Phase 0). Exercises a required `text` field and a
 * `text` result (the archived branch name).
 */
const manifest: OpManifest = {
  id: "branch.archive",
  domain: "branch",
  op: "archive",
  label: "Branch: Archive",
  description: "Archives a branch locally and on the remote, preventing further commits.",
  command: "branch_archive",
  args: [
    {
      name: "branch",
      kind: "text",
      label: "Branch",
      description: "The branch name to archive.",
      required: true,
      placeholder: "old-feature",
    },
  ],
  resultKind: "text",
  keywords: ["archive", "delete", "remove", "cleanup"],
};

export default manifest;
