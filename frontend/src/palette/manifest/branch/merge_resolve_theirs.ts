import type { OpManifest } from "../../types";

/** Resolve merge conflicts by taking the incoming ("theirs") version. */
const manifest: OpManifest = {
  id: "branch.merge_resolve_theirs",
  domain: "branch",
  op: "merge_resolve_theirs",
  label: "Branch: Resolve Merge (Theirs)",
  description:
    "Resolve conflicts on the given paths by taking the incoming (theirs) version.",
  command: "branch_merge_resolve_theirs",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description: "Conflicted paths to resolve as theirs (one per line).",
      required: true,
    },
  ],
  resultKind: "json",
  keywords: ["branch", "merge", "resolve", "theirs", "conflict", "incoming"],
};

export default manifest;
