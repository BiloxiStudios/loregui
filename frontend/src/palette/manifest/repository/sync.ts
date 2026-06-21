import type { OpManifest } from "../../types";

/** Sync the working repository with its remote (pull + apply). */
const manifest: OpManifest = {
  id: "repository.sync",
  domain: "repository",
  op: "sync",
  label: "Repository: Sync",
  description:
    "Sync the current repository with its remote, pulling and applying the latest changes.",
  command: "sync",
  args: [],
  resultKind: "void",
  keywords: ["sync", "pull", "update", "remote", "fetch"],
};

export default manifest;
