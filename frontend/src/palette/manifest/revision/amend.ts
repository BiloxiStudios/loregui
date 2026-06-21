import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for revision.amend.
 *
 * Amends the most recent revision by replacing its commit message.
 * Returns the amended revision hash, number, and branch.
 */
const manifest: OpManifest = {
  id: "revision.amend",
  domain: "revision",
  op: "amend",
  label: "Revision: Amend",
  description: "Amend the most recent revision's commit message.",
  command: "revision_amend",
  args: [
    {
      name: "message",
      kind: "text",
      label: "Message",
      required: true,
      placeholder: "New commit message",
    },
  ],
  resultKind: "json",
  keywords: ["amend", "edit", "reword", "message", "revision"],
};

export default manifest;
