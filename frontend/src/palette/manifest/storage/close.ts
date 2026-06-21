import type { OpManifest } from "../../types";

/** Release a storage handle previously returned by Storage: Open. */
const manifest: OpManifest = {
  id: "storage.close",
  domain: "storage",
  op: "close",
  label: "Storage: Close",
  description:
    "Release an open storage handle, draining in-flight ops and flushing pending writes.",
  command: "storage_close",
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
  keywords: ["storage", "close", "handle", "release"],
  surface: "panel",
};

export default manifest;
