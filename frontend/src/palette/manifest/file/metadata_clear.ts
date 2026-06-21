import type { OpManifest } from "../../types";

/**
 * Reference manifest entry (Phase 0). Exercises the `text` field and a
 * simple object result.
 */
const manifest: OpManifest = {
  id: "file.metadata_clear",
  domain: "file",
  op: "metadata_clear",
  label: "File: Metadata Clear",
  description: "Clear all metadata associated with a file.",
  command: "metadata_clear",
  args: [
    {
      name: "path",
      kind: "text",
      label: "Path",
      description: "Path to the file whose metadata will be cleared.",
      required: true,
      placeholder: "src/foo.txt",
    },
  ],
  resultKind: "object",
  keywords: ["metadata", "clear", "remove", "delete"],
};

export default manifest;
