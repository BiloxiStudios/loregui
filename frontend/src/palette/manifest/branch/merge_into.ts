import type { OpManifest } from "../../types";

/** Merge the current branch into a specified target branch. */
const manifest: OpManifest = {
  id: "branch.merge_into",
  domain: "branch",
  op: "merge_into",
  label: "Branch: Merge Into",
  description:
    "Merge the current branch into a specified target branch with a commit message.",
  command: "branch_merge_into",
  args: [
    {
      name: "branch",
      kind: "text",
      label: "Target branch",
      description: "Branch to merge the current branch into.",
      required: true,
      placeholder: "e.g. main",
    },
    {
      name: "branchId",
      kind: "text",
      label: "Target branch id",
      description: "Optional explicit id of the target branch.",
      required: false,
      default: "",
    },
    {
      name: "message",
      kind: "text",
      label: "Message",
      description: "Commit message for the merge.",
      required: false,
      default: "",
    },
    {
      name: "link",
      kind: "text",
      label: "Link",
      description: "Optional merge link identifier.",
      required: false,
      default: "",
    },
    {
      name: "ignoreLinks",
      kind: "boolean",
      label: "Ignore links",
      description: "Ignore link records during the merge.",
      required: false,
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["branch", "merge", "into", "target", "integrate"],
};

export default manifest;
