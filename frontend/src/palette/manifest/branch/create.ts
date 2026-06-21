import type { OpManifest } from "../../types";

/** Create a new branch with an optional category and explicit id. */
const manifest: OpManifest = {
  id: "branch.create",
  domain: "branch",
  op: "create",
  label: "Branch: Create",
  description:
    "Create a new branch, optionally under a category and with an explicit branch id.",
  command: "branch_create",
  args: [
    {
      name: "branch",
      kind: "text",
      label: "Branch name",
      description: "Name of the branch to create.",
      required: true,
      placeholder: "e.g. feature/my-work",
    },
    {
      name: "category",
      kind: "text",
      label: "Category",
      description: "Optional branch category (e.g. main, dev).",
      required: false,
      default: "",
    },
    {
      name: "id",
      kind: "text",
      label: "Branch id",
      description:
        "Optional explicit branch id (hex-encoded 16-byte context); leave empty to auto-generate.",
      required: false,
      default: "",
    },
  ],
  resultKind: "json",
  keywords: ["branch", "create", "new", "fork"],
};

export default manifest;
