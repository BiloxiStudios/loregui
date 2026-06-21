import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for layer.add.
 *
 * Mounts a source repository as a layer inside the current repository at a
 * target path. The source is mounted starting at `sourcePath` and revisions are
 * matched between the two repositories via the `metadata` key.
 */
const manifest: OpManifest = {
  id: "layer.add",
  domain: "layer",
  op: "add",
  label: "Layer: Add",
  description:
    "Mount a source repository as a layer inside the current repository at a target path.",
  command: "layer_add",
  args: [
    {
      name: "targetPath",
      kind: "text",
      label: "Target path",
      description:
        "Path in the current repository where the layer should be placed.",
      required: true,
      placeholder: "e.g. /layers/shared-assets",
    },
    {
      name: "sourceRepository",
      kind: "text",
      label: "Source repository",
      description: "Repository to add as a layer (URL or repository id).",
      required: true,
      placeholder: "e.g. https://example.com/repo",
    },
    {
      name: "sourcePath",
      kind: "text",
      label: "Source path",
      description:
        "Path in the source repository where the layer should start. Leave empty for the repository root.",
      required: false,
      default: "",
      placeholder: "/",
    },
    {
      name: "metadata",
      kind: "text",
      label: "Metadata key",
      description:
        "Metadata key used to match revisions between the repositories. Optional.",
      required: false,
      default: "",
      placeholder: "e.g. branch",
    },
  ],
  resultKind: "json",
  keywords: ["layer", "add", "mount", "overlay", "subrepo", "compose"],
};

export default manifest;
