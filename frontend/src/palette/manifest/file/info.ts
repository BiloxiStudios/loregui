import type { OpManifest } from "../../types";

/**
 * Palette manifest for `file.info`.
 *
 * Retrieves metadata for one or more files: size, hash, staged status,
 * modification flags, and optionally local/filtered sizes.
 */
const manifest: OpManifest = {
  id: "file.info",
  domain: "file",
  op: "info",
  label: "File: Info",
  description:
    "Show metadata for files — size, hash, flags, and staging status.",
  command: "file_info",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description: "Repository-relative paths to query (one per line).",
      required: true,
      placeholder: "src/main.rs\nassets/model.fbx",
    },
    {
      name: "revision",
      kind: "text",
      label: "Revision",
      description:
        "Revision specifier to query (leave empty for working copy).",
      required: false,
      placeholder: "",
    },
    {
      name: "local",
      kind: "boolean",
      label: "Include Local Info",
      description: "Calculate the filtered local filesystem hash and size.",
      default: false,
    },
    {
      name: "filtered",
      kind: "boolean",
      label: "Include Filtered Size",
      description: "Calculate the filtered repository size.",
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["file", "info", "metadata", "size", "hash", "status", "stat"],
};

export default manifest;
