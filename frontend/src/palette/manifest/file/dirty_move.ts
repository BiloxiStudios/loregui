import type { OpManifest } from "../../types";

/**
 * Reference manifest entry (Phase 0). Exercises two required `text` fields and a
 * structured JSON result.
 */
const manifest: OpManifest = {
  id: "file.dirty_move",
  domain: "file",
  op: "dirty_move",
  label: "File: Dirty Move",
  description: "Mark a file as moved in the staging area.",
  command: "file_dirty_move",
  args: [
    {
      name: "fromPath",
      kind: "text",
      label: "From Path",
      description: "Original path of the file to move.",
      required: true,
      placeholder: "src/foo.txt",
    },
    {
      name: "toPath",
      kind: "text",
      label: "To Path",
      description: "New destination path.",
      required: true,
      placeholder: "src/bar.txt",
    },
  ],
  resultKind: "json",
  keywords: ["move", "rename", "relocate"],
};

export default manifest;
