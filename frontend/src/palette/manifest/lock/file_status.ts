import type { OpManifest } from "../../types";

/**
 * Palette manifest for `lock.file_status`.
 *
 * Returns lock status for specified files on a branch.
 */
const manifest: OpManifest = {
  id: "lock.file_status",
  domain: "lock",
  op: "file_status",
  label: "Lock: File Status",
  description:
    "Check the lock status of one or more files — shows owner and lock time.",
  command: "lock_file_status",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description: "File paths to check, one per line.",
      required: true,
      placeholder: "Content/Characters/hero.uasset\nContent/Maps/main.umap",
    },
    {
      name: "branch",
      kind: "text",
      label: "Branch",
      description: "Branch to check locks on; leave empty for current branch.",
      required: false,
      placeholder: "e.g. main",
    },
  ],
  resultKind: "json",
  keywords: ["lock", "status", "check", "info", "file", "who"],
};

export default manifest;
