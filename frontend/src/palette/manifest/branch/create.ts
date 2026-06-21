import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for branch.create.
 *
 * Creates a new branch with the given name and category.
 */
const manifest: OpManifest = {
  id: "branch.create",
  domain: "branch",
  op: "create",
  label: "Branch: Create",
  description:
    "Create a new branch with the given name and optional category.",
  command: "branch_create",
  args: [
    {
      name: "branch",
      kind: "text",
      label: "Branch Name",
      description: "Name of the branch to create.",
      required: true,
      placeholder: "e.g. feature/my-feature",
    },
    {
      name: "category",
      kind: "text",
      label: "Category",
      description: "Branch category (e.g. feature, release, hotfix).",
      required: false,
      placeholder: "e.g. feature",
    },
    {
      name: "id",
      kind: "text",
      label: "Branch ID",
      description:
        "Explicit branch ID (hex-encoded 16-byte context); leave empty to auto-generate.",
      required: false,
    },
  ],
  resultKind: "json",
  keywords: ["branch", "create", "new", "fork"],
};

export default manifest;
