import type { OpManifest } from "../../types";

/**
 * Manifest entry for file.dump operation.
 *
 * Dumps the binary content of a file by path or address.
 * Returns entries with address, flags, sizes, and match status.
 */
const manifest: OpManifest = {
  id: "file.dump",
  domain: "file",
  op: "dump",
  label: "File: Dump",
  description: "Dump the binary content of a file by path or address.",
  command: "dump",
  args: [
    {
      name: "address",
      kind: "text",
      label: "Address",
      required: false,
      placeholder: "Content address (takes precedence over path)",
    },
    {
      name: "path",
      kind: "text",
      label: "Path",
      required: false,
      placeholder: "Repository-relative path to dump",
    },
  ],
  resultKind: "json",
  keywords: ["dump", "content", "inspect", "binary"],
};

export default manifest;
