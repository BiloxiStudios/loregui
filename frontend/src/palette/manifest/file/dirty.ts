import type { OpManifest } from "../../types";

/** Mark one or more paths as dirty (locally modified) for the next commit. */
const manifest: OpManifest = {
  id: "file.dirty",
  domain: "file",
  op: "dirty",
  label: "File: Mark Dirty",
  description:
    "Mark one or more paths as dirty (locally modified) so they are picked up by the next commit.",
  command: "file_dirty",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description: "Paths to mark as dirty (one per line).",
      required: true,
      placeholder: "src/foo.txt\nsrc/bar.txt",
    },
  ],
  resultKind: "json",
  keywords: ["file", "dirty", "modified", "touch", "edit", "change"],
};

export default manifest;
