import type { OpManifest } from "../../types";

/**
 * Read a mutable key from an open store. Absent keys report AddressNotFound.
 * Palette-only power-user surface (IA: rare / scriptable storage ops).
 */
const manifest: OpManifest = {
  id: "storage.mutable_load",
  domain: "storage",
  op: "mutable_load",
  label: "Storage: Mutable Load",
  description:
    "Read a mutable key's value (hash) from an open store. Absent keys return AddressNotFound on the item. Optional remote routing uses the handle's remote session.",
  command: "storage_mutable_load",
  requiresRepository: false,
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
  keywords: ["storage", "mutable", "load", "kv", "read"],
  surface: "palette",
};

export default manifest;
