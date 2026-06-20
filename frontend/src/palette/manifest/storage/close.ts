import type { OpManifest } from "../../types";

/**
 * Storage close manifest. Releases a storage handle acquired via storage open.
 * The upstream call unregisters the handle and returns any diagnostic log messages.
 */
const manifest: OpManifest = {
  id: "storage.close",
  domain: "storage",
  op: "close",
  label: "Storage: Close",
  description:
    "Release a storage handle acquired via storage open. Invalidates the handle and drains in-flight ops.",
  command: "storage_close",
  args: [
    {
      name: "handle",
      kind: "number",
      label: "Handle ID",
      description: "Handle ID of the open storage instance to close.",
      required: true,
    },
  ],
  resultKind: "json",
  keywords: ["close", "release", "storage", "handle"],
};

export default manifest;
