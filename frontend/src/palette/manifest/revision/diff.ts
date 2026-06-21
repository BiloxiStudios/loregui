import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for revision.diff.
 *
 * Computes file-level differences between two revisions, showing
 * which files were added, deleted, moved, copied, or kept.
 */
const manifest: OpManifest = {
  id: "revision.diff",
  domain: "revision",
  op: "diff",
  label: "Revision: Diff",
  description: "Show file-level differences between two revisions.",
  command: "revision_diff",
  args: [
    {
      name: "revisionSource",
      kind: "text",
      label: "Source Revision",
      description: "Revision to diff from.",
      required: true,
      placeholder: "e.g. abc123def",
    },
    {
      name: "revisionTarget",
      kind: "text",
      label: "Target Revision",
      description: "Revision to diff to; empty for current working state.",
      required: false,
      placeholder: "e.g. def456abc",
    },
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description: "Restrict diff to these repository-relative paths; empty for all files.",
      required: false,
    },
  ],
  resultKind: "json",
  keywords: ["diff", "compare", "changes", "files", "revision"],
};

export default manifest;
