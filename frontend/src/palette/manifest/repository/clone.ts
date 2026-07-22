import type { OpManifest } from "../../types";

/**
 * Manifest entry for `repository.clone`. Requires the repository URL to clone
 * from and a local destination path. Invokes the `repository_clone` Tauri
 * command.
 */
const manifest: OpManifest = {
  id: "repository.clone",
  domain: "repository",
  op: "clone",
  label: "Repository: Clone",
  description: "Clone a remote repository to a local directory.",
  command: "repository_clone",
  requiresRepository: false,
  args: [
    {
      name: "url",
      kind: "text",
      label: "Repository URL",
      required: true,
      placeholder: "lore://host/repo",
    },
    {
      name: "dest",
      kind: "text",
      label: "Destination",
      required: true,
      placeholder: "/path/to/clone",
    },
  ],
  resultKind: "void",
  keywords: ["clone", "checkout", "download", "fetch"],
};

export default manifest;
