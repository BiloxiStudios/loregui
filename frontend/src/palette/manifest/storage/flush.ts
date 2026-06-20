import type { OpManifest } from "../../types";

/**
 * Storage flush manifest. Flushes pending writes through an open storage handle.
 * Disk-backed stores call fsync; in-memory stores no-op.
 */
const manifest: OpManifest = {
  id: "storage.flush",
  domain: "storage",
  op: "flush",
  label: "Storage: Flush",
  description:
    "Flush pending writes through an open storage handle. Disk-backed stores call fsync.",
  command: "storage_flush",
  args: [
    {
      name: "handle",
      kind: "number",
      label: "Handle ID",
      description: "Handle ID of the open storage instance to flush.",
      required: true,
    },
  ],
  resultKind: "json",
  keywords: ["flush", "fsync", "storage", "write"],
};

export default manifest;
