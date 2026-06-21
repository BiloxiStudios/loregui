import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for revision.find_local.
 *
 * Finds revisions matching a metadata key/value pair or revision number,
 * searching only the local repository (no remote dispatch).
 */
const manifest: OpManifest = {
  id: "revision.find_local",
  domain: "revision",
  op: "find_local",
  label: "Revision: Find Local",
  description:
    "Find revisions by metadata key/value or revision number (local only).",
  command: "revision_find_local",
  args: [
    {
      name: "key",
      kind: "text",
      label: "Metadata Key",
      required: false,
      placeholder: "e.g. tag, author",
      description: "Metadata key to search for; leave empty to search by number.",
    },
    {
      name: "value",
      kind: "text",
      label: "Metadata Value",
      required: false,
      placeholder: "e.g. release-1.0",
      description: "Value to match against the metadata key.",
    },
    {
      name: "number",
      kind: "number",
      label: "Revision Number",
      required: false,
      default: 0,
      placeholder: "0",
      description: "Revision number to find; used when key is empty. 0 disables.",
    },
  ],
  resultKind: "json",
  keywords: ["find", "search", "local", "revision", "metadata", "lookup"],
};

export default manifest;
