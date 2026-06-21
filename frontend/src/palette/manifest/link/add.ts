import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for `link add`.
 *
 * Adds a new link to a linked repository at the specified path.
 */
const manifest: OpManifest = {
  id: "link.add",
  domain: "link",
  op: "add",
  label: "Link: Add",
  description: "Add a new link to a linked repository at the specified path.",
  command: "link_add",
  args: [
    {
      name: "link",
      kind: "text",
      label: "Link",
      description: "Link repository URL or identifier.",
      required: true,
      placeholder: "https://example.com/repo or repo-name",
    },
    {
      name: "linkPath",
      kind: "text",
      label: "Link path",
      description: "Path within this repository where the link is added.",
      required: true,
      placeholder: "deps/external",
    },
    {
      name: "sourcePath",
      kind: "text",
      label: "Source path",
      description: "Source path within the linked repository; / or \\ means the root.",
      required: false,
      placeholder: "/",
    },
    {
      name: "pin",
      kind: "text",
      label: "Pin",
      description: "Branch or revision to set the link pin at.",
      required: false,
      placeholder: "main",
    },
    {
      name: "disableBranching",
      kind: "boolean",
      label: "Disable branching",
      description: "Disable automatic branch creation in the linked repository.",
      required: false,
    },
  ],
  resultKind: "json",
  keywords: ["link", "add", "bind", "connect"],
};

export default manifest;
