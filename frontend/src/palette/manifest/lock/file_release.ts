import type { OpManifest } from "../../types";

/**
 * Palette manifest for `lock.file_release`.
 *
 * Releases exclusive locks on one or more files.
 */
const manifest: OpManifest = {
  id: "lock.file_release",
  domain: "lock",
  op: "file_release",
  label: "Lock: Release Files",
  description: "Release exclusive file locks for the specified paths.",
  command: "lock_file_release",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description: "File paths to unlock, one per line.",
      required: true,
      placeholder: "Content/Characters/hero.uasset\nContent/Maps/main.umap",
    },
    {
      name: "branch",
      kind: "text",
      label: "Branch",
      description: "Branch the locks were acquired on; leave empty for current branch.",
      required: false,
      placeholder: "e.g. main",
    },
    {
      name: "owner",
      kind: "text",
      label: "Owner",
      description: "Lock owner name.",
      required: true,
      placeholder: "e.g. user@example.com",
    },
    {
      name: "ownerId",
      kind: "text",
      label: "Owner ID",
      description: "Lock owner ID.",
      required: true,
      placeholder: "e.g. 12345",
    },
  ],
  resultKind: "json",
  keywords: ["lock", "release", "unlock", "free", "file"],
};

export default manifest;
