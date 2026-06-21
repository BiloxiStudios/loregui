import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for revision.cherry_pick_resolve.
 *
 * Marks the specified conflicted paths as resolved during an in-progress
 * cherry-pick, indicating the user has manually resolved the conflicts.
 * The paths array accepts one repository-relative path per entry.
 */
const manifest: OpManifest = {
  id: "revision.cherry_pick_resolve",
  domain: "revision",
  op: "cherry_pick_resolve",
  label: "Revision: Cherry-Pick Resolve",
  description:
    "Mark conflicted paths as resolved during an in-progress cherry-pick.",
  command: "revision_cherry_pick_resolve",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description:
        "Repository-relative paths to mark as resolved (one per line).",
      required: true,
      placeholder: "src/main.rs\nREADME.md",
    },
  ],
  resultKind: "json",
  keywords: [
    "cherry-pick",
    "resolve",
    "conflict",
    "merge",
    "cherry",
    "pick",
  ],
};

export default manifest;
