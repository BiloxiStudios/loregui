import type { OpManifest } from "../../types";

/** Show configured shared stores and whether they're used automatically. */
const manifest: OpManifest = {
  id: "shared_store.info",
  domain: "shared_store",
  op: "info",
  label: "Shared Store: Info",
  description:
    "List the configured shared stores — their URLs, paths, existence, and auto-use setting.",
  command: "shared_store_info",
  args: [],
  resultKind: "json",
  keywords: ["shared", "store", "info", "list", "status"],
  surface: "panel",
};

export default manifest;
