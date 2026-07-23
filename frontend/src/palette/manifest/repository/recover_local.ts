import type { OpManifest } from "../../types";

/**
 * Palette manifest for `repository.recover_local` (SBAI-5499).
 *
 * Recovers an unreadable local working tree: the current tree is preserved
 * and a fresh local copy is checked out. Destructive — the manifest system
 * has no confirm gate, so the description carries the warning and the
 * Changes-panel card (UrcStatusCard) confirms before invoking.
 */
const manifest: OpManifest = {
  id: "repository.recover_local",
  domain: "repository",
  op: "recover_local",
  label: "Repository: Recover Local",
  description:
    "Recover an unreachable local working tree — preserves the current tree and checks out a fresh local copy. Destructive: confirm before running.",
  command: "repository_recover_local",
  args: [
    {
      name: "newDir",
      kind: "text",
      label: "New directory",
      description:
        "Directory to check the recovered copy out into; leave empty to recover in place.",
      required: false,
      default: "",
      placeholder: "/path/to/recovered-repo",
    },
  ],
  resultKind: "json",
  keywords: ["recover", "repair", "unreachable", "working tree", "restore"],
};

export default manifest;
