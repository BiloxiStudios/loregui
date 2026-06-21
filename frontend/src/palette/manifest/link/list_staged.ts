import type { OpManifest } from "../../types";

/**
 * Manifest entry for `link list_staged`.
 *
 * Lists links with staged changes in the current repository.
 * Returns a JSON result with link count and details of each link with staged changes.
 * Follows the Phase 0 pattern: no-arg op whose result is a typed object.
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
  keywords: ["link", "staged", "changes", "deps"],
};

export default manifest;
