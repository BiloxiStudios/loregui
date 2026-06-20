import type { OpManifest } from "../../types";

/**
 * Storage get_metadata manifest. Fetches fragment metadata (flags, payload size,
 * content size) for one or more content-addressed items without transferring
 * payload bytes.
 */
const manifest: OpManifest = {
  id: "storage.get_metadata",
  domain: "storage",
  op: "get_metadata",
  label: "Storage: Get Metadata",
  description:
    "Fetch fragment metadata (flags, payload size, content size) for items without transferring payload bytes.",
  command: "storage_get_metadata",
  args: [
    {
      name: "handle",
      kind: "number",
      label: "Handle ID",
      description: "Handle ID returned by a prior storage open call.",
      required: true,
    },
    {
      name: "items",
      kind: "text",
      label: "Items (JSON)",
      description:
        'Array of items to look up: [{"id": 1, "partition": "<hex>", "address": "<hash>-<context>"}]',
      required: true,
      placeholder: '[{"id":1,"partition":"...","address":"..."}]',
    },
  ],
  resultKind: "json",
  keywords: ["metadata", "storage", "fragment", "size", "flags"],
};

export default manifest;
