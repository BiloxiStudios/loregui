import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for `link list`.
 *
 * Lists all linked repositories registered in the current repository.
 */
const manifest: OpManifest = {
  id: "link.list",
  domain: "link",
  op: "list",
  label: "Link: List",
  description: "List all linked repositories registered in the current repository.",
  command: "link_list",
  args: [],
  resultKind: "json",
  keywords: ["link", "list", "show", "display"],
};

export default manifest;
