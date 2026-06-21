import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for dependency.add.
 *
 * Adds file dependencies to the current repository. Each source file
 * can have multiple dependency targets with optional classification tags
 * (e.g. "texture", "compile"). Cycle detection is performed unless force
 * is set.
 */
const manifest: OpManifest = {
  id: "dependency.add",
  domain: "dependency",
  op: "add",
  label: "Dependency: Add",
  description:
    "Add file dependencies to the current repository with optional tags.",
  command: "dependency_add",
  args: [
    {
      name: "sources",
      kind: "text",
      label: "Sources (JSON)",
      description:
        'JSON array of {path, dependencies: [{dependency, tags}]}. Example: [{"path":"a.txt","dependencies":[{"dependency":"b.txt","tags":["texture"]}]}]',
      required: true,
      placeholder:
        '[{"path":"src.fbx","dependencies":[{"dependency":"tex.png","tags":["texture"]}]}]',
    },
    {
      name: "force",
      kind: "boolean",
      label: "Force",
      description: "Skip cycle detection.",
      required: false,
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["dependency", "add", "link", "edge", "graph"],
};

export default manifest;
