import type { OpManifest } from "../../types";

/** Flush pending writes through an open storage handle (fsync on disk stores). */
const manifest: OpManifest = {
  id: "storage.flush",
  domain: "storage",
  op: "flush",
  label: "Storage: Flush",
  description:
    "Flush pending writes through an open storage handle, fsyncing disk-backed stores.",
  command: "storage_flush",
  args: [
    {
      name: "handle",
      kind: "number",
      label: "Handle",
      description: "Handle id returned by Storage: Open.",
      required: true,
      placeholder: "1",
    },
  ],
  resultKind: "json",
  keywords: ["storage", "flush", "fsync", "handle", "persist"],
  surface: "panel",
};

export default manifest;
