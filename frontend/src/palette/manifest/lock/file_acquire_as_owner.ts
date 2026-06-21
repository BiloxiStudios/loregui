import type { OpManifest } from "../../types";

/**
 * Palette manifest for `lock.file_acquire_as_owner`.
 *
 * Acquires exclusive locks on one or more files on behalf of a specified owner.
 */
const manifest: OpManifest = {
  id: "lock.file_acquire_as_owner",
  domain: "lock",
  op: "file_acquire_as_owner",
  label: "Lock: Acquire Files as Owner",
  description:
    "Acquire exclusive file locks on behalf of another user (admin operation).",
  command: "lock_file_acquire_as_owner",
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
    {
      name: "owner",
      kind: "text",
      label: "Owner",
      description: "User ID of the lock owner.",
      required: true,
      placeholder: "e.g. user@example.com",
    },
  ],
  resultKind: "json",
  keywords: ["lock", "acquire", "owner", "admin", "delegate", "file"],
};

export default manifest;
