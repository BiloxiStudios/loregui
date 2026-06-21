import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for `link update`.
 *
 * Updates an existing link in the repository.
 * NOTE: This op is currently a stub in lore-vm and needs implementation.
 */
const manifest: OpManifest = {
  id: "link.update",
  domain: "link",
  op: "update",
  label: "Link: Update",
  description: "Update an existing link in the repository.",
  command: "link_update",
  args: [
    {
      name: "linkPath",
      kind: "text",
      label: "Link path",
      description: "Path of the link to update.",
      required: true,
      placeholder: "deps/external",
    },
  ],
  resultKind: "json",
  keywords: ["link", "update", "modify", "change"],
};

export default manifest;
