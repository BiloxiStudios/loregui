import type { OpManifest } from "../../types";

/** List recent revisions in the current branch's history. */
const manifest: OpManifest = {
  id: "revision.log",
  domain: "revision",
  op: "log",
  label: "Revision: Log",
  description:
    "List the most recent revisions on the current branch, newest first.",
  command: "log",
  args: [
    {
      name: "limit",
      kind: "number",
      label: "Limit",
      description: "Maximum number of revisions to return.",
      required: true,
      default: 50,
      placeholder: "50",
    },
  ],
  resultKind: "json",
  keywords: ["log", "history", "revisions", "commits", "recent"],
};

export default manifest;
