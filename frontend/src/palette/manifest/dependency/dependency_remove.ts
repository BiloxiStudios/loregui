import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for dependency.remove.
 *
 * Removes file dependencies from the current repository. If tags are
 * specified, only those tags are removed from the edge; if no tags are
 * given the entire dependency edge is removed. Back-references on target
 * files are updated automatically.
 */
const manifest: OpManifest = {
  id: "dependency.remove",
  domain: "dependency",
  op: "remove",
  label: "Dependency: Remove",
  description:
    "Remove file dependencies (or specific tags) from the current repository.",
  command: "dependency_remove",
  args: [
    {
      name: "sources",
      kind: "text",
      label: "Sources (JSON)",
      description:
        'JSON array of {path, dependencies: [{dependency, tags}]}. Empty tags removes the entire edge. Example: [{"path":"a.txt","dependencies":[{"dependency":"b.txt"}]}]',
      required: true,
      placeholder:
        '[{"path":"src.fbx","dependencies":[{"dependency":"tex.png"}]}]',
    },
  ],
  resultKind: "json",
  keywords: ["dependency", "remove", "delete", "unlink", "edge", "graph"],
};

export default manifest;
