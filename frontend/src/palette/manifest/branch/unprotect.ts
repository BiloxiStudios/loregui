import type { OpManifest } from "../../types";

/**
 * Reference manifest entry (Phase 0). A single-arg op whose result is a typed
 * object — exercises the text-field + JSON-result path of the palette.
 */
const manifest: OpManifest = {
  id: "branch.unprotect",
  domain: "branch",
  op: "unprotect",
  label: "Branch: Unprotect",
  description:
    "Remove write protection from a branch, re-allowing direct commits.",
  command: "branch_unprotect",
  args: [
    {
      name: "branch",
      kind: "text",
      label: "Branch",
      description: "The branch name to unprotect.",
      required: true,
      placeholder: "main",
    },
  ],
  resultKind: "json",
  keywords: ["unprotect", "unlock", "write", "protection"],
};

export default manifest;
