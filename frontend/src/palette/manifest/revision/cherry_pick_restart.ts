import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for revision.cherry_pick_restart.
 *
 * Re-materialises the specified paths for resolution during an in-progress
 * cherry-pick conflict, discarding any partial resolution work so the user
 * can start over on those paths.
 */
const manifest: OpManifest = {
  id: "revision.cherry_pick_restart",
  domain: "revision",
  op: "cherry_pick_restart",
  label: "Revision: Cherry-Pick Restart",
  description:
    "Restart resolution for specified paths during an in-progress cherry-pick conflict.",
  command: "revision_cherry_pick_restart",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      required: true,
      description:
        "Repository-relative paths to re-materialise for resolution (one per line).",
      placeholder: "src/main.rs",
    },
  ],
  resultKind: "json",
  keywords: [
    "cherry-pick",
    "restart",
    "conflict",
    "resolve",
    "redo",
    "rematerialise",
  ],
};

export default manifest;
