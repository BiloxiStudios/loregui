import type { OpManifest } from "../../types";

/**
 * Storage put manifest. Writes one or more content-addressed buffers
 * to an open storage handle. Each item is hashed and stored independently.
 */
const manifest: OpManifest = {
  id: "storage.put",
  domain: "storage",
  op: "put",
  label: "Storage: Put",
  description:
    "Write one or more content-addressed buffers to an open storage handle. Each item is hashed and stored independently.",
  command: "storage_put",
  args: [
    {
      name: "handle",
      kind: "number",
      label: "Handle ID",
      description: "Handle ID of an already-open store (from storage open).",
      required: true,
    },
    {
      name: "key",
      kind: "text",
      label: "Key",
      description: "Opaque key for this put operation.",
      required: true,
    },
    {
      name: "data",
      kind: "text",
      label: "Data (base64)",
      description: "Bytes to store, encoded as base64 string.",
      required: true,
    },
  ],
  resultKind: "void",
  keywords: ["put", "write", "store", "storage", "content"],
};

export default manifest;
