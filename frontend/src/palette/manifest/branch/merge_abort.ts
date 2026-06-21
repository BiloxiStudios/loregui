import type { OpManifest } from "../../types";

/** Abort an in-progress branch merge and discard its staged state. */
const manifest: OpManifest = {
  id: "branch.merge_abort",
  domain: "branch",
  op: "merge_abort",
  label: "Branch: Abort Merge",
  description:
    "Abort an in-progress branch merge, discarding its staged merge state.",
  command: "branch_merge_abort",
  args: [
    {
      name: "link",
      kind: "text",
      label: "Link",
      description: "Merge link identifier to abort.",
      required: false,
      default: "",
    },
    {
      name: "ignoreLinks",
      kind: "boolean",
      label: "Ignore links",
      description: "Ignore link records while aborting.",
      required: false,
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["branch", "merge", "abort", "cancel", "discard"],
};

export default manifest;
