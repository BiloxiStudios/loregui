import type { OpManifest } from "../../types";

/**
 * Palette manifest for `file.hash`.
 *
 * Computes the content hash (BLAKE3) and size of one or more files in
 * the repository. Returns one entry per file with path, size, and hash.
 */
const manifest: OpManifest = {
  id: "file.hash",
  domain: "file",
  op: "hash",
  label: "File: Hash",
  description:
    "Compute the BLAKE3 content hash and size of one or more files.",
  command: "file_hash",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description: "File paths to hash (relative to repository root).",
      required: true,
      placeholder: "src/foo.txt\nassets/bar.png",
    },
  ],
  resultKind: "json",
  keywords: ["hash", "blake3", "checksum", "size", "digest", "file"],
};

export default manifest;
