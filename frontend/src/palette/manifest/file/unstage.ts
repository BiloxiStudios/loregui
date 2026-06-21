import type { OpManifest } from "../../types";

/**
 * Manifest entry for `file.unstage` — remove paths from the staged changeset.
 * Mirrors the Phase 0 reference entry for `file.stage`.
 */
const manifest: OpManifest = {
  id: "file.unstage",
  domain: "file",
  op: "unstage",
  label: "File: Unstage",
  description: "Remove one or more paths from the staged changeset.",
  command: "unstage",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description: "One path per line to unstage.",
      required: true,
      placeholder: "src/foo.txt\nsrc/bar.txt",
    },
  ],
  resultKind: "json",
  keywords: ["unstage", "discard", "remove", "changeset", "index"],
};

export default manifest;
