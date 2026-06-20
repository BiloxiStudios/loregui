import type { OpManifest } from "../../types";

/**
 * Storage upload manifest. Pushes one or more locally-stored,
 * not-yet-durable content entries to the remote store attached to an
 * open storage handle.
 */
const manifest: OpManifest = {
  id: "storage.upload",
  domain: "storage",
  op: "upload",
  label: "Storage: Upload",
  description:
    "Push locally-stored content entries to the remote store attached to an open storage handle.",
  command: "storage_upload",
  args: [
    {
      name: "handle",
      kind: "number",
      label: "Handle ID",
      description: "Handle ID of an already-open store (must have remote config).",
      required: true,
    },
    {
      name: "items",
      kind: "text",
      label: "Items (JSON)",
      description:
        'Array of items to upload: [{"id": 1, "partition": "<hex>", "address": "<hash>-<context>"}]',
      required: true,
      placeholder: '[{"id":1,"partition":"...","address":"..."}]',
    },
  ],
  resultKind: "json",
  keywords: ["upload", "remote", "sync", "storage", "push"],
};

export default manifest;
