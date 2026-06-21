import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for `link remove`.
 *
 * Removes a link from the repository at the specified path.
 */
const manifest: OpManifest = {
  id: "link.remove",
  domain: "link",
  op: "remove",
  label: "Link: Remove",
  description: "Remove a link from the repository at the specified path.",
  command: "link_remove",
  args: [
    {
      name: "linkPath",
      kind: "text",
      label: "Link path",
      description: "Path within this repository where the link is removed.",
      required: true,
      placeholder: "deps/external",
    },
  ],
  resultKind: "json",
  keywords: ["link", "remove", "delete", "unlink"],
};

export default manifest;
