import type { OpManifest } from "../../types";

/** Read a metadata value stored on a branch by key. */
const manifest: OpManifest = {
  id: "branch.metadata_get",
  domain: "branch",
  op: "metadata_get",
  label: "Branch: Get Metadata",
  description: "Read the metadata value stored on a branch under a given key.",
  command: "branch_metadata_get",
  args: [
    {
      name: "branch",
      kind: "text",
      label: "Branch",
      description: "Branch whose metadata to read.",
      required: true,
      placeholder: "e.g. main",
    },
    {
      name: "key",
      kind: "text",
      label: "Key",
      description: "Metadata key to read.",
      required: true,
      placeholder: "e.g. description",
    },
  ],
  resultKind: "json",
  keywords: ["branch", "metadata", "get", "read", "key", "property"],
};

export default manifest;
