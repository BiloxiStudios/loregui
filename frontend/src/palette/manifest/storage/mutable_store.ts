import type { OpManifest } from "../../types";

/**
 * Write a mutable key-value pair on an open store. Empty/zero value removes the key.
 * Palette-only power-user surface (IA: rare / scriptable storage ops).
 */
const manifest: OpManifest = {
  id: "storage.mutable_store",
  domain: "storage",
  op: "mutable_store",
  label: "Storage: Mutable Store",
  description:
    "Write a mutable key-value pair (hashes) on an open store. Empty or all-zero value removes the key. Optional remote routing uses the handle's remote session.",
  command: "storage_mutable_store",
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
      description: "32-hex-char partition (non-zero).",
      required: true,
      placeholder: "00000000000000000000000000000001",
    },
    {
      name: "key",
      kind: "text",
      label: "Key (hash)",
      description: "64-hex-char key hash.",
      required: true,
      placeholder: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    },
    {
      name: "value",
      kind: "text",
      label: "Value (hash)",
      description: "64-hex-char value hash. Empty or zero removes the key.",
      required: false,
      placeholder: "(empty removes key)",
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
      label: "Remote",
      description: "Route via the remote StorageSession (requires open with remote URL).",
      required: false,
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["storage", "mutable", "store", "kv", "pointer"],
  surface: "palette",
};

export default manifest;
