import type { OpManifest } from "../../types";

/**
 * Manifest entry for dependency.dependency_add.
 *
 * Adds file dependencies to the current repository. Each entry defines
 * a source file and its dependencies with optional tags (e.g., "texture",
 * "compile"). Cycle detection is performed unless force is set.
 *
 * Note: The sources parameter uses a JSON text field due to the nested
 * array structure [{path, dependencies: [{dependency, tags: []}]}]
 * not being expressible with current FieldKind types. Future schema-driven
 * forms will render proper nested controls.
 *
 * Example sources JSON:
 * [
 *   {
 *     "path": "/src/main.txt",
 *     "dependencies": [
 *       {"dependency": "/assets/texture.png", "tags": ["texture"]},
 *       {"dependency": "/lib/common.txt", "tags": ["compile"]}
 *     ]
 *   }
 * ]
 */
const manifest: OpManifest = {
  id: "dependency.dependency_add",
  domain: "dependency",
  op: "dependency_add",
  label: "Dependency: Add",
  description: "Add file dependencies to repository sources.",
  command: "dependency_add",
  args: [
    {
      name: "sources",
      kind: "text",
      label: "Sources (JSON)",
      required: true,
      description: "Array of {path, dependencies: [{dependency, tags: []}]}. See manifest docstring for example.",
      placeholder: '[{"path":"/src/file.txt","dependencies":[{"dependency":"/dep.txt","tags":["compile"]}]}]',
    },
    {
      name: "force",
      kind: "boolean",
      label: "Force",
      description: "Skip cycle detection when true.",
      required: false,
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["dependency", "add", "link", "edge"],
};

export default manifest;
