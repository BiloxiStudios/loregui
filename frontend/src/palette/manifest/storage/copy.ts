import type { OpManifest } from "../../types";

/**
 * Storage copy manifest. Copies content between partition/context tuples
 * in the same open store. Each item runs independently.
 */
const manifest: OpManifest = {
  id: "storage.copy",
  domain: "storage",
  op: "copy",
  label: "Storage: Copy",
  description:
    "Copy content between (partition, context) tuples in the same open store. Each item runs independently.",
  command: "storage_copy",
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
      label: "Copy Items (JSON)",
      description:
        'Array of copy items: [{"id": 1, "sourcePartition": "<hex>", "targetPartition": "<hex>", "sourceAddress": "<hash>-<context>", "targetContext": "<hex>"}]',
      required: true,
      placeholder: '[{"id":1,"sourcePartition":"...","targetPartition":"...","sourceAddress":"...","targetContext":"..."}]',
    },
  ],
  resultKind: "json",
  keywords: ["copy", "storage", "partition", "address"],
};

export default manifest;
