import type { OpManifest } from "../../types";

/**
 * Palette manifest for `lock.file_acquire`.
 *
 * Acquires exclusive locks on one or more files on the current branch.
 */
const manifest: OpManifest = {
  id: "lock.file_acquire",
  domain: "lock",
  op: "file_acquire",
  label: "Lock: Acquire Files",
  description:
    "Acquire exclusive locks on one or more files for the current user.",
  command: "lock_file_acquire",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description: "File paths to lock, one per line.",
      required: true,
      placeholder: "Content/Characters/hero.uasset\nContent/Maps/main.umap",
    },
    {
      name: "branch",
      kind: "text",
      label: "Branch",
      description: "Branch to acquire locks on; leave empty for current branch.",
      required: false,
      placeholder: "e.g. main",
    },
  ],
  resultKind: "json",
  keywords: ["lock", "acquire", "file", "exclusive", "claim"],
};

export default manifest;
