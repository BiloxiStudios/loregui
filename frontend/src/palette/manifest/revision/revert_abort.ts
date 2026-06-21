import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for revision.revert_abort.
 *
 * Aborts an in-progress revert and restores the working directory to its
 * prior state. Takes no arguments beyond the repository context.
 */
const manifest: OpManifest = {
  id: "revision.revert_abort",
  domain: "revision",
  op: "revert_abort",
  label: "Revision: Abort Revert",
  description:
    "Abort an in-progress revert operation and restore the working directory.",
  command: "revert_abort",
  args: [],
  resultKind: "json",
  keywords: ["revert", "abort", "cancel", "undo"],
};

export default manifest;
