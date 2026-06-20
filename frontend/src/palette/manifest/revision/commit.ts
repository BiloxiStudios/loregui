import type { OpManifest } from "../../types";

/**
 * Reference manifest entry (Phase 0). Exercises a required `text` field and a
 * `text` result (the new revision hash).
 */
const manifest: OpManifest = {
  id: "revision.commit",
  domain: "revision",
  op: "commit",
  label: "Revision: Commit",
  description: "Commit the staged changes with a message.",
  command: "commit",
  args: [
    {
      name: "message",
      kind: "text",
      label: "Message",
      required: true,
      placeholder: "Describe the change",
    },
  ],
  resultKind: "text",
  keywords: ["commit", "save", "snapshot"],
};

export default manifest;
