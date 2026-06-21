import type { OpManifest } from "../../types";

/**
 * Manifest for `repository.release` — releases cached store references for the
 * current repository path.
 */
const manifest: OpManifest = {
  id: "repository.release",
  domain: "repository",
  op: "release",
  label: "Repository: Release",
  description: "Release cached store references for the current repository path.",
  command: "repository_release",
  args: [],
  resultKind: "json",
  keywords: ["release", "store", "cache", "unlock", "free"],
};

export default manifest;
