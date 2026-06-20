import type { OpManifest } from "../../types";

/**
 * Reference manifest entry (Phase 0). Exercises the `string-list` field and a
 * `void` result.
 */
const manifest: OpManifest = {
  id: "file.stage",
  domain: "file",
  op: "stage",
  label: "File: Stage",
  description: "Stage one or more paths for the next commit.",
  command: "stage",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description: "One path per line.",
      required: true,
      placeholder: "src/foo.txt\nsrc/bar.txt",
    },
  ],
  resultKind: "void",
  keywords: ["add", "stage", "index"],
};

export default manifest;
