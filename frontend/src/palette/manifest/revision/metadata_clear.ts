import type { OpManifest } from "../../types";

/**
 * Manifest entry for revision.metadata_clear.
 *
 * Clears metadata keys from a revision. If revision is empty, clears from
 * the current HEAD (committed). Use metadata_get to read keys and
 * metadata_set to write them; metadata_clear removes them entirely.
 */
const manifest: OpManifest = {
  id: "revision.metadata_clear",
  domain: "revision",
  op: "metadata_clear",
  label: "Revision: Metadata Clear",
  description: "Clear metadata keys from a revision.",
  command: "metadata_clear",
  args: [
    {
      name: "keys",
      kind: "string-list",
      label: "Keys",
      description: "One metadata key per line.",
      required: true,
      placeholder: "change-request\nreviewed-by",
    },
    {
      name: "revision",
      kind: "text",
      label: "Revision",
      description: "Leave empty for current HEAD.",
      required: false,
      placeholder: "abc123def",
    },
  ],
  resultKind: "void",
  keywords: ["metadata", "clear", "delete", "remove"],
};

export default manifest;
