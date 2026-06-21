import type { OpManifest } from "../../types";

/** Look up fragment metadata (flags, payload/content size) without fetching bytes. */
const manifest: OpManifest = {
  id: "storage.get_metadata",
  domain: "storage",
  op: "get_metadata",
  label: "Storage: Get Metadata",
  description:
    "Fetch a fragment's metadata (flags, payload size, content size) by partition and address — no bytes transferred.",
  command: "storage_get_metadata",
  args: [
    {
      name: "handle",
      kind: "number",
      label: "Handle",
      description: "Handle id returned by Storage: Open.",
      required: true,
      placeholder: "1",
    },
    {
      name: "partition",
      kind: "text",
      label: "Partition",
      description: "32-hex-char partition namespace (the zero partition is rejected).",
      required: true,
      placeholder: "00000000000000000000000000000001",
    },
    {
      name: "address",
      kind: "text",
      label: "Address",
      description: "Content address: <hash> or <hash>-<context>.",
      required: true,
      placeholder: "<hash>-<context>",
    },
  ],
  resultKind: "json",
  keywords: ["storage", "metadata", "fragment", "size", "info"],
  surface: "panel",
};

export default manifest;
