import type { OpManifest } from "../../types";

/**
 * Palette manifest for `repository.store_immutable_query`.
 *
 * Queries the local immutable store for fragments matching a given address.
 * Returns matching entries with address, status, payload/content sizes, and flags.
 */
const manifest: OpManifest = {
  id: "repository.store_immutable_query",
  domain: "repository",
  op: "store_immutable_query",
  label: "Repository: Query Immutable Store",
  description:
    "Query the local immutable store for fragments matching an address.",
  command: "store_immutable_query",
  args: [
    {
      name: "address",
      kind: "text",
      label: "Fragment Address",
      description: "The fragment address to query in the immutable store.",
      required: true,
      placeholder: "abc123",
    },
    {
      name: "recurse",
      kind: "boolean",
      label: "Recurse",
      description: "When true, recurse into and query subfragments.",
      default: false,
    },
  ],
  resultKind: "json",
  keywords: [
    "store",
    "immutable",
    "query",
    "fragment",
    "address",
    "payload",
    "content",
  ],
};

export default manifest;
