import type { OpManifest } from "../../types";

/** Push a locally-stored fragment to the store's configured remote. */
const manifest: OpManifest = {
  id: "storage.upload",
  domain: "storage",
  op: "upload",
  label: "Storage: Upload",
  description:
    "Push a locally-stored fragment to the remote attached to an open store (handle must have a remote configured).",
  command: "storage_upload",
  requiresRepository: false,
  args: [
    {
      name: "handle",
      kind: "number",
      label: "Handle",
      description: "Handle id returned by Storage: Open (must have a remote configured).",
      required: true,
      placeholder: "1",
    },
    {
      name: "partition",
      kind: "text",
      label: "Partition",
      description: "32-hex-char partition namespace of the local content.",
      required: true,
      placeholder: "00000000000000000000000000000001",
    },
    {
      name: "address",
      kind: "text",
      label: "Address",
      description: "Content address to push (<hash> or <hash>-<context>).",
      required: true,
      placeholder: "<hash>-<context>",
    },
  ],
  resultKind: "json",
  keywords: ["storage", "upload", "remote", "push", "durable"],
  surface: "palette",
};

export default manifest;
