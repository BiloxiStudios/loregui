import type { OpManifest } from "../../types";

/**
 * Manifest for file.dirty_copy operation.
 *
 * Creates a new staged destination node flagged as DirtyCopy. The source
 * node is unchanged — this is a metadata-only staging operation with no
 * filesystem I/O.
 */
const manifest: OpManifest = {
  id: "file.dirty_copy",
  domain: "file",
  op: "dirty_copy",
  label: "File: Dirty Copy",
  description:
    "Stage a file copy without touching the filesystem. Creates a DirtyCopy node.",
  command: "file_dirty_copy",
  args: [
    {
      name: "fromPath",
      kind: "text",
      label: "From Path",
      description: "Source path of the file to copy.",
      required: true,
      placeholder: "src/main.rs",
    },
    {
      name: "toPath",
      kind: "text",
      label: "To Path",
      description: "Destination path for the copy.",
      required: true,
      placeholder: "src/main_copy.rs",
    },
  ],
  resultKind: "json",
  keywords: ["copy", "dirty", "stage", "duplicate"],
};

export default manifest;
