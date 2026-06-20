import type { OpManifest } from "../../types";

/**
 * Storage get manifest. Reads one or more content-addressed buffers
 * from an open store handle. Each item can emit per-fragment events or
 * a single reassembled buffer.
 */
const manifest: OpManifest = {
  id: "storage.get",
  domain: "storage",
  op: "get",
  label: "Storage: Get",
  description:
    "Read one or more content-addressed buffers from an open store handle. Each item runs independently.",
  command: "storage_get",
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
      label: "Get Items (JSON)",
      description:
        'Array of items to retrieve: [{"id": 1, "partition": "<hex>", "address": "<hash>-<context>", "streaming": false, "localCache": true}]',
      required: true,
      placeholder: '[{"id":1,"partition":"...","address":"...","streaming":false,"localCache":true}]',
    },
  ],
  resultKind: "json",
  keywords: ["get", "read", "fetch", "storage", "content"],
};

export default manifest;
