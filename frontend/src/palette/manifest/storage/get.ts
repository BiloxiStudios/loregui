import type { OpManifest } from "../../types";

/** Retrieve a content-addressed buffer from an open store by its session key. */
const manifest: OpManifest = {
  id: "storage.get",
  domain: "storage",
  op: "get",
  label: "Storage: Get",
  description:
    "Read a content-addressed buffer from the open store using a session key returned by a prior Storage: Put.",
  command: "storage_get",
  requiresRepository: false,
  args: [
    {
      name: "key",
      kind: "text",
      label: "Key",
      description: "Session key returned by a prior Storage: Put call.",
      required: true,
      placeholder: "my-key",
    },
  ],
  resultKind: "json",
  keywords: ["storage", "get", "read", "fetch", "content", "address", "key"],
  surface: "panel",
};

export default manifest;
