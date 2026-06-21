import type { OpManifest } from "../../types";

/**
 * Manifest entry for `revision.revert_resolve_mine`.
 *
 * Resolves the specified conflicted paths during an in-progress revert by
 * keeping the "mine" (local/current-branch) version of each file.
 */
const manifest: OpManifest = {
  id: "revision.revert_resolve_mine",
  domain: "revision",
  op: "revert_resolve_mine",
  label: "Revision: Revert Resolve Mine",
  description:
    "Resolve revert conflicts on the given paths by keeping the local (mine) version.",
  command: "revert_resolve_mine",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description: "Repository-relative paths to resolve in favor of mine (local).",
      required: true,
      placeholder: "src/main.rs\nREADME.md",
    },
  ],
  resultKind: "json",
  keywords: ["revert", "resolve", "mine", "conflict", "local"],
};

export default manifest;
