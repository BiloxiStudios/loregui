import type { OpManifest } from "../../types";

/** Copy content between partitions in the same open store (hash preserved). */
const manifest: OpManifest = {
  id: "storage.copy",
  domain: "storage",
  op: "copy",
  label: "Storage: Copy",
  description:
    "Copy a fragment from one partition to another in the same store, preserving its content hash.",
  command: "storage_copy",
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
      name: "sourcePartition",
      kind: "text",
      label: "Source partition",
      description: "32-hex-char source partition namespace.",
      required: true,
      placeholder: "00000000000000000000000000000001",
    },
    {
      name: "sourceAddress",
      kind: "text",
      label: "Source address",
      description: "Content address of the fragment to copy (<hash> or <hash>-<context>).",
      required: true,
      placeholder: "<hash>-<context>",
    },
    {
      name: "targetPartition",
      kind: "text",
      label: "Target partition",
      description: "32-hex-char destination partition namespace.",
      required: true,
      placeholder: "00000000000000000000000000000002",
    },
    {
      name: "targetContext",
      kind: "text",
      label: "Target context",
      description: "Optional 32-hex-char dedup context for the destination. Empty uses the zero context.",
      placeholder: "(optional)",
    },
  ],
  resultKind: "json",
  keywords: ["storage", "copy", "partition", "fragment"],
  surface: "palette",
};

export default manifest;
