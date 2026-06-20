import type { OpManifest } from "../../types";

/**
 * Shared-store info manifest.
 *
 * Returns information about the configured default shared stores including
 * their remote URLs, filesystem paths, existence status, and whether they're
 * used automatically.
 */
const manifest: OpManifest = {
  id: "shared_store.info",
  domain: "shared_store",
  op: "info",
  label: "Shared Store: Info",
  description:
    "Show information about configured default shared stores — remote URLs, paths, existence status, and auto-use setting.",
  command: "shared_store_info",
  args: [],
  resultKind: "json",
  keywords: ["shared", "store", "info", "status", "list"],
};

export default manifest;
