import type { OpManifest } from "../../types";

/**
 * Palette manifest for `file.metadata_list`.
 *
 * Lists all metadata key/value pairs associated with a file at a given
 * revision. Returns entries with key, value, and type (address, boolean,
 * binary, context, hash, numeric, string).
 */
const manifest: OpManifest = {
  id: "file.metadata_list",
  domain: "file",
  op: "metadata_list",
  label: "File: List Metadata",
  description:
    "List all metadata key/value pairs for a file at a given revision.",
  command: "file_metadata_list",
  args: [
    {
      name: "path",
      kind: "text",
      label: "Path",
      description: "Path to the file to list metadata for.",
      required: true,
      placeholder: "src/main.rs",
    },
    {
      name: "revision",
      kind: "text",
      label: "Revision",
      description: "Revision to query; leave empty for the current revision.",
      required: false,
      default: "",
      placeholder: "e.g. abc123def",
    },
  ],
  resultKind: "json",
  keywords: ["metadata", "list", "file", "key", "value", "attributes"],
};

export default manifest;
