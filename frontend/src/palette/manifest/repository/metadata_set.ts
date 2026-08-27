import type { OpManifest } from "../../types";

/**
 * Palette manifest for `repository.metadata_set`.
 *
 * Sets one or more metadata key-value pairs on the repository.
 * Keys and values are parallel arrays; formats default to "string".
 */
const manifest: OpManifest = {
  id: "repository.metadata_set",
  domain: "repository",
  op: "metadata_set",
  label: "Repository: Set Metadata",
  description: "Set metadata key-value pairs on the repository.",
  command: "repository_metadata_set",
  args: [
    {
      name: "keys",
      kind: "string-list",
      label: "Keys",
      description: "Metadata keys to set (one per line).",
      required: true,
    },
    {
      name: "values",
      kind: "string-list",
      label: "Values",
      description:
        "Values for each key (one per line, parallel to keys).",
      required: true,
    },
    {
      name: "formats",
      kind: "string-list",
      label: "Formats",
      description:
        'Format for each key: "address", "boolean", "binary", "context", "hash", "numeric", or "string". Defaults to "string" for missing entries.',
      required: false,
      default: [],
    },
  ],
  resultKind: "json",
  keywords: ["metadata", "set", "write", "key", "value", "update"],
};

export default manifest;
