import type { OpManifest } from "../../types";

/** Merge a named branch into the current branch in one step. */
const manifest: OpManifest = {
  id: "branch.merge",
  domain: "branch",
  op: "merge",
  label: "Branch: Merge",
  description: "Merge a named branch into the current branch.",
  command: "merge_branch",
  args: [
    {
      name: "name",
      kind: "text",
      label: "Branch",
      description: "Name of the branch to merge into the current branch.",
      required: true,
      placeholder: "e.g. feature/my-work",
    },
  ],
  resultKind: "void",
  keywords: ["branch", "merge", "integrate", "combine"],
};

export default manifest;
