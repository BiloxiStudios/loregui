import type { OpManifest } from "../../types";

/**
 * Manifest for revision.find — searches revisions by metadata key/value or revision number.
 * Finds matching revisions across local and remote shared stores.
 */
const manifest: OpManifest = {
  id: "revision.find",
  domain: "revision",
  op: "find",
  label: "Revision: Find",
  description: "Find revisions by metadata key/value or revision number, searching local and remote stores.",
  command: "revision_find",
  args: [
    {
      name: "key",
      kind: "text",
      label: "Metadata Key",
      description: "Search by this metadata key (e.g. 'tag', 'author'). Leave empty to search by revision number.",
      required: false,
      placeholder: "tag",
    },
    {
      name: "value",
      kind: "text",
      label: "Metadata Value",
      description: "Match this value for the given key.",
      required: false,
      placeholder: "release-1.0",
    },
    {
      name: "number",
      kind: "number",
      label: "Revision Number",
      description: "Search by revision number when key is empty.",
      required: false,
      default: 0,
      placeholder: "42",
    },
  ],
  resultKind: "json",
  keywords: ["search", "query", "metadata", "revision-number"],
};

export default manifest;
