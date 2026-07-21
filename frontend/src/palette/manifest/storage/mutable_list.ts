import type { OpManifest } from "../../types";

/**
 * List mutable key-value pairs for a partition/type. Local-only — remote is rejected.
 * Palette-only power-user surface (IA: rare / scriptable storage ops).
 */
const manifest: OpManifest = {
  id: "storage.mutable_list",
  domain: "storage",
  op: "mutable_list",
  label: "Storage: Mutable List",
  description:
    "List mutable key-value pairs of a given type on the local store. Remote targeting is rejected (local-only). Zero partition lists every accessible partition.",
  command: "storage_mutable_list",
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
      description: "32-hex-char partition. Empty/zero lists every accessible partition.",
      required: false,
      placeholder: "00000000000000000000000000000001",
    },
    {
      name: "keyType",
      kind: "text",
      label: "Key type",
      description: "Upstream KeyType camelCase name (default untyped).",
      required: false,
      placeholder: "untyped",
      default: "untyped",
    },
    {
      name: "remote",
      kind: "boolean",
      label: "Remote (will fail)",
      description:
        "Force remote routing. Upstream rejects this: mutable_list is local-only.",
      required: false,
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["storage", "mutable", "list", "kv", "enumerate"],
  surface: "palette",
};

export default manifest;
