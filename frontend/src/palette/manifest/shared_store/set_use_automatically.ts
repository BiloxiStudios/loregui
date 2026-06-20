import type { OpManifest } from "../../types";

/**
 * Shared-store set_use_automatically manifest.
 *
 * Sets whether the configured default shared store should be used automatically.
 */
const manifest: OpManifest = {
  id: "shared_store.set_use_automatically",
  domain: "shared_store",
  op: "set_use_automatically",
  label: "Shared Store: Set Auto-Use",
  description:
    "Set whether to automatically use the configured default shared store for this repository.",
  command: "shared_store_set_use_automatically",
  args: [
    {
      name: "enabled",
      kind: "boolean",
      label: "Enable Auto-Use",
      description: "When true, the default shared store is used automatically.",
      required: false,
      default: false,
    },
  ],
  resultKind: "void",
  keywords: ["shared", "store", "auto", "automatic", "enable", "disable"],
};

export default manifest;
