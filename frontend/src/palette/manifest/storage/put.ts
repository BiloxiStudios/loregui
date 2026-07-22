import type { OpManifest } from "../../types";

/**
 * Store a content-addressed buffer in an open store. The key is an opaque
 * caller-chosen label used to correlate a later get or obliterate call.
 */
const manifest: OpManifest = {
  id: "storage.put",
  domain: "storage",
  op: "put",
  label: "Storage: Put",
  description:
    "Write a content-addressed buffer to an open store by key. The key lets you retrieve or obliterate the data later.",
  command: "storage_put",
  requiresRepository: false,
  args: [
    {
      name: "key",
      kind: "text",
      label: "Key",
      description: "Opaque label that correlates this write with a later get or obliterate.",
      required: true,
      placeholder: "my-data-key",
    },
    {
      name: "data",
      kind: "text",
      label: "Data (JSON byte array)",
      description:
        "The bytes to store, as a JSON array of numbers (e.g. [72, 101, 108, 108, 111]).",
      required: true,
      placeholder: "[72, 101, 108, 108, 111]",
    },
  ],
  resultKind: "void",
  keywords: ["storage", "put", "write", "store", "content", "address"],
  surface: "palette",
};

export default manifest;
