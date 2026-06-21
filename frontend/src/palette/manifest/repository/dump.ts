import type { OpManifest } from "../../types";

/**
 * Palette manifest for `repository.dump`.
 *
 * Dumps the internal state tree of the repository for diagnostic purposes.
 * Shows revision summary, node tree, and diagnostic log messages.
 */
const manifest: OpManifest = {
  id: "repository.dump",
  domain: "repository",
  op: "dump",
  label: "Repository: Dump State",
  description:
    "Dump the internal state tree of the repository for diagnostics.",
  command: "repository_dump",
  args: [
    {
      name: "revision",
      kind: "text",
      label: "Revision",
      description:
        "Revision to dump; leave empty to use the current revision.",
      required: false,
      default: "",
      placeholder: "HEAD",
    },
    {
      name: "path",
      kind: "text",
      label: "Path",
      description:
        "Repository-relative path to start from; leave empty to dump from root.",
      required: false,
      default: "",
      placeholder: "/",
    },
    {
      name: "maxDepth",
      kind: "number",
      label: "Max Depth",
      description: "Maximum tree traversal depth (0 = unlimited).",
      required: false,
      default: 0,
    },
  ],
  resultKind: "json",
  keywords: [
    "dump",
    "state",
    "tree",
    "diagnostic",
    "debug",
    "inspect",
    "node",
    "internal",
  ],
};

export default manifest;
