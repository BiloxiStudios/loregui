import type { OpManifest } from "../../types";

/** Read a file from disk and store it at a content-addressed location. */
const manifest: OpManifest = {
  id: "storage.put_file",
  domain: "storage",
  op: "put_file",
  label: "Storage: Put File",
  description:
    "Read a file from disk and store its contents in an open store, returning the content address.",
  command: "storage_put_file",
  args: [
    {
      name: "handle",
      kind: "number",
      label: "Handle",
      description: "Handle id returned by Storage: Open.",
      required: true,
      placeholder: "1",
    },
    {
      name: "partition",
      kind: "text",
      label: "Partition",
      description: "32-hex-char partition namespace (the zero partition is rejected).",
      required: true,
      placeholder: "00000000000000000000000000000001",
    },
    {
      name: "path",
      kind: "text",
      label: "File path",
      description: "Filesystem path of the file to read and store.",
      required: true,
      placeholder: "/path/to/file.bin",
    },
    {
      name: "context",
      kind: "text",
      label: "Context",
      description: "Optional 32-hex-char dedup context. Empty uses the zero context.",
      placeholder: "(optional)",
    },
    {
      name: "remoteWrite",
      kind: "boolean",
      label: "Remote write",
      description: "Also push the fragment to the configured remote store.",
    },
    {
      name: "localCache",
      kind: "boolean",
      label: "Local cache priority",
      description: "Tag the fragment for local-cache retention priority.",
    },
  ],
  resultKind: "json",
  keywords: ["storage", "put", "file", "upload", "store", "write"],
  surface: "palette",
};

export default manifest;
