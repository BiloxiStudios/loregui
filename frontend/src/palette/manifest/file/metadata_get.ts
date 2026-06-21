import type { OpManifest } from "../../types";

/**
 * Manifest entry for file.metadata_get.
 *
 * Retrieves a single metadata value for a file by key at a given revision.
 * Returns the key, value, and type of the requested metadata entry.
 *
 * Metadata values have typed representations:
 * - Address: content identifier hash
 * - Boolean: true/false
 * - Binary: base64-encoded raw bytes (JSON array format)
 * - Context: context identifier
 * - Hash: hash value
 * - Numeric: unsigned integer
 * - String: text value
 *
 * Use metadata_set to write keys and metadata_list to enumerate all keys
 * on a file.
 */
const manifest: OpManifest = {
  id: "file.metadata_get",
  domain: "file",
  op: "metadata_get",
  label: "File: Get Metadata",
  description: "Get a single metadata value for a file by key.",
  command: "metadata_get",
  args: [
    {
      name: "path",
      kind: "text",
      label: "Path",
      description: "Path to the file to get metadata for.",
      required: true,
      placeholder: "src/main.rs",
    },
    {
      name: "key",
      kind: "text",
      label: "Key",
      description: "Metadata key to retrieve.",
      required: true,
      placeholder: "author",
    },
    {
      name: "revision",
      kind: "text",
      label: "Revision",
      description: "Revision to get metadata for; empty string uses current revision.",
      required: false,
      placeholder: "Leave empty for current revision",
    },
  ],
  resultKind: "json",
  keywords: ["metadata", "get", "file", "read"],
};

export default manifest;
