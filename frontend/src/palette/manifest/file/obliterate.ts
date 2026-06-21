import type { OpManifest } from "../../types";

/** Permanently remove a file's content at a given address from the store. */
const manifest: OpManifest = {
  id: "file.obliterate",
  domain: "file",
  op: "obliterate",
  label: "File: Obliterate",
  description:
    "Permanently remove a file's content at a specific address from the store. This is irreversible.",
  command: "file_obliterate",
  args: [
    {
      name: "path",
      kind: "text",
      label: "Path",
      description: "Repository path of the file to obliterate.",
      required: true,
      placeholder: "src/secret.txt",
    },
    {
      name: "address",
      kind: "text",
      label: "Address",
      description: "Content address (hash) of the file content to obliterate.",
      required: true,
      placeholder: "content hash",
    },
  ],
  resultKind: "json",
  keywords: ["file", "obliterate", "purge", "delete", "remove", "destroy"],
};

export default manifest;
