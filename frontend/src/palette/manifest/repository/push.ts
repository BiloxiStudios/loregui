import type { OpManifest } from "../../types";

/** Push local commits to the repository's remote. */
const manifest: OpManifest = {
  id: "repository.push",
  domain: "repository",
  op: "push",
  label: "Repository: Push",
  description: "Push local commits on the current branch to the remote.",
  command: "push",
  args: [],
  resultKind: "void",
  keywords: ["push", "upload", "remote", "publish", "send"],
};

export default manifest;
