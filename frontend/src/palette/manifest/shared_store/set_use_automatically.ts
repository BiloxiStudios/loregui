import type { OpManifest } from "../../types";

/** Toggle whether repositories use the default shared store automatically. */
const manifest: OpManifest = {
  id: "shared_store.set_use_automatically",
  domain: "shared_store",
  op: "set_use_automatically",
  label: "Shared Store: Set Auto-Use",
  description:
    "Enable or disable automatic use of the configured default shared store for repositories.",
  command: "shared_store_set_use_automatically",
  args: [
    {
      name: "enabled",
      kind: "boolean",
      label: "Use automatically",
      description: "When on, the default shared store is consulted automatically.",
    },
  ],
  resultKind: "void",
  keywords: ["shared", "store", "automatic", "default", "toggle"],
  surface: "panel",
};

export default manifest;
