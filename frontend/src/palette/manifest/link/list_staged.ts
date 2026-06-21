import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for `link list_staged`.
 *
 * Lists links with staged changes in the current repository.
 */
const manifest: OpManifest = {
  id: "link.list_staged",
  domain: "link",
  op: "list_staged",
  label: "Link: List Staged",
  description: "List all links with staged changes in the current repository.",
  command: "link_list_staged",
  args: [],
  resultKind: "json",
  keywords: ["link", "list", "staged", "changes", "pending"],
};

export default manifest;
