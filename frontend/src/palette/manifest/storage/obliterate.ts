import type { OpManifest } from "../../types";

/**
 * Storage obliterate manifest. Permanently deletes content at
 * (partition, address) from an open storage handle. Runs local and
 * remote obliteration in parallel when configured.
 */
const manifest: OpManifest = {
  id: "storage.obliterate",
  domain: "storage",
  op: "obliterate",
  label: "Storage: Obliterate",
  description:
    "Permanently delete content at (partition, address) from an open storage handle. Runs local and remote in parallel.",
  command: "storage_obliterate",
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
        'Array of items to obliterate: [{"id": 1, "partition": "<hex>", "address": "<hash>-<context>"}]',
      required: true,
      placeholder: '[{"id":1,"partition":"...","address":"..."}]',
    },
  ],
  resultKind: "json",
  keywords: ["obliterate", "delete", "remove", "storage"],
};

export default manifest;
