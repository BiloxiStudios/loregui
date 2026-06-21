import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for link.add.
 *
 * Adds a new link to a linked repository at the specified path.
 * Links connect one repository to another, pulling content from
 * a source path in the linked repo into a local link path.
 */
const manifest: OpManifest = {
  id: "link.add",
  domain: "link",
  op: "add",
  label: "Link: Add",
  description:
    "Add a link to a remote repository at the specified path.",
  command: "link_add",
  args: [
    {
      name: "link",
      kind: "text",
      label: "Link URL",
      description: "URL or identifier of the repository to link.",
      required: true,
      placeholder: "https://example.com/repo",
    },
    {
      name: "linkPath",
      kind: "text",
      label: "Link Path",
      description: "Path within this repository where the link is added.",
      required: true,
      placeholder: "deps/external",
    },
    {
      name: "sourcePath",
      kind: "text",
      label: "Source Path",
      description:
        'Path within the linked repository to pull from. "/" means the root.',
      required: false,
      default: "/",
    },
    {
      name: "pin",
      kind: "text",
      label: "Pin",
      description: "Branch or revision to pin the link at.",
      required: false,
      default: "",
    },
    {
      name: "disableBranching",
      kind: "boolean",
      label: "Disable Branching",
      description:
        "Disable automatic branch creation in the linked repository.",
      required: false,
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["link", "add", "remote", "connect", "external", "dependency"],
};

export default manifest;
