import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for dependency.list.
 *
 * Lists file dependencies (or dependents in reverse mode) at a given
 * revision. Supports recursive traversal, tag filtering, and depth limits.
 */
const manifest: OpManifest = {
  id: "dependency.list",
  domain: "dependency",
  op: "list",
  label: "Dependency: List",
  description:
    "List file dependencies or dependents at a given revision.",
  command: "dependency_list",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description: "File paths to query dependencies for.",
      required: true,
      default: [],
    },
    {
      name: "revision",
      kind: "text",
      label: "Revision",
      description: "Revision to query at; empty for current.",
      required: false,
      placeholder: "e.g. abc123def",
    },
    {
      name: "recursive",
      kind: "boolean",
      label: "Recursive",
      description: "Follow transitive dependencies recursively.",
      required: false,
      default: false,
    },
    {
      name: "reverse",
      kind: "boolean",
      label: "Reverse (Dependents)",
      description: "Return dependents (reverse lookup) instead of dependencies.",
      required: false,
      default: false,
    },
    {
      name: "tags",
      kind: "string-list",
      label: "Tags",
      description: "Filter results by these tags.",
      required: false,
      default: [],
    },
    {
      name: "depthLimit",
      kind: "number",
      label: "Depth Limit",
      description: "Maximum recursion depth; 0 for unlimited.",
      required: false,
      default: 0,
    },
  ],
  resultKind: "json",
  keywords: ["dependency", "list", "query", "dependents", "graph"],
};

export default manifest;
