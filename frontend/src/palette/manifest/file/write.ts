import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for `file.write`.
 *
 * Writes file content from the repository to a destination filesystem path.
 * Can resolve files by path+revision or by direct content address.
 */
const manifest: OpManifest = {
  id: "file.write",
  domain: "file",
  op: "write",
  label: "File: Write",
  description:
    "Write a file from the repository to a destination path on the filesystem.",
  command: "file_write",
  args: [
    {
      name: "address",
      kind: "text",
      label: "Address",
      description:
        "Content address to write; takes precedence over path when set.",
      required: false,
      placeholder: "e.g. abc123def456",
    },
    {
      name: "path",
      kind: "text",
      label: "Path",
      description:
        "Repository-relative path to the file (used when address is empty).",
      required: false,
      placeholder: "e.g. src/main.rs",
    },
    {
      name: "revision",
      kind: "text",
      label: "Revision",
      description: "Revision of the file to write (used with path).",
      required: false,
      placeholder: "e.g. abc123",
    },
    {
      name: "output",
      kind: "text",
      label: "Output Path",
      description: "Destination filesystem path to write the file to.",
      required: true,
      placeholder: "/tmp/output.rs",
    },
  ],
  resultKind: "text",
  keywords: ["write", "export", "output", "file", "save", "copy"],
};

export default manifest;
