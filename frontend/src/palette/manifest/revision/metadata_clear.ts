import type { OpManifest } from "../../types";

/**
 * Manifest entry for revision.metadata_clear.
 *
 * Clears ALL user-defined metadata from the current revision. The upstream lore
 * operation takes no arguments — it removes every metadata key on the current
 * revision (it does not select individual keys). Use metadata_get/metadata_list
 * to read keys and metadata_set to write them; metadata_clear removes them all.
 */
const manifest: OpManifest = {
  id: "revision.metadata_clear",
  domain: "revision",
  op: "metadata_clear",
  label: "Revision: Metadata Clear",
  description: "Clear all metadata from the current revision.",
  command: "revision_metadata_clear",
  args: [],
  resultKind: "json",
  keywords: ["metadata", "clear", "delete", "remove"],
};

export default manifest;
