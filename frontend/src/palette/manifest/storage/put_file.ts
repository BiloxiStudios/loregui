import type { OpManifest } from "../../types";

/**
 * Storage put_file manifest. Reads one or more files from disk and stores
 * their contents at content-addressed locations in an open storage handle.
 */
const manifest: OpManifest = {
  id: "storage.put_file",
  domain: "storage",
  op: "put_file",
  label: "Storage: Put File",
  description:
    "Read one or more files from disk and store their contents at content-addressed locations.",
  command: "storage_put_file",
  args: [
    {
      name: "handle",
      kind: "number",
      label: "Handle ID",
      description: "Handle ID of an already-open store (from storage open).",
      required: true,
    },
    {
      name: "items",
      kind: "text",
      label: "File Items (JSON)",
      description:
        'Array of files to store: [{"id": 1, "partition": "<hex>", "context": "<hex>", "path": "/path/to/file", "remoteWrite": false, "localCache": false, "fixedSizeChunk": 0}]',
      required: true,
      placeholder: '[{"id":1,"partition":"...","context":"...","path":"/path/to/file"}]',
    },
  ],
  resultKind: "json",
  keywords: ["put_file", "file", "storage", "upload", "disk"],
};

export default manifest;
