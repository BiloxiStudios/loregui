import type { OpManifest } from "../../types";

/** Begin a merge of another branch into the current branch. */
const manifest: OpManifest = {
  id: "branch.merge_start",
  domain: "branch",
  op: "merge_start",
  label: "Branch: Start Merge",
  description:
    "Begin merging another branch into the current branch, optionally committing the result.",
  command: "branch_merge_start",
  args: [
    {
      name: "branch",
      kind: "text",
      label: "Source branch",
      description: "Branch to merge into the current branch.",
      required: true,
      placeholder: "e.g. feature/my-work",
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
      name: "noCommit",
      kind: "boolean",
      label: "No commit",
      description: "Stage the merge without creating a commit.",
      required: false,
      default: false,
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
  keywords: ["branch", "merge", "start", "begin", "integrate"],
};

export default manifest;
