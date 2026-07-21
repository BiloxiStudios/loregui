import type { OpManifest } from "../../types";

/**
 * Conditionally swap a mutable key when current value matches expected.
 * Palette-only power-user surface (IA: rare / scriptable storage ops).
 */
const manifest: OpManifest = {
  id: "storage.mutable_compare_and_swap",
  domain: "storage",
  op: "mutable_compare_and_swap",
  label: "Storage: Mutable Compare-and-Swap",
  description:
    "Conditionally update a mutable key when its current value matches expected (empty expected matches an absent key). Returns previous value and whether the swap applied. Remote-capable.",
  command: "storage_mutable_compare_and_swap",
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
      name: "expected",
      kind: "text",
      label: "Expected (hash)",
      description: "Expected current value. Empty/zero matches an absent key.",
      required: false,
      placeholder: "(empty = absent key)",
    },
    {
      name: "value",
      kind: "text",
      label: "New value (hash)",
      description: "Value to store when the swap applies. Empty/zero removes the key.",
      required: false,
      placeholder: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
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
  keywords: ["storage", "mutable", "cas", "compare", "swap", "atomic"],
  surface: "palette",
};

export default manifest;
