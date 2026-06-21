import type { OpManifest } from "../../types";

/** Clone a remote repository URL to a local destination. */
const manifest: OpManifest = {
  id: "repository.clone_url",
  domain: "repository",
  op: "clone_url",
  label: "Repository: Clone from URL",
  description:
    "Clone a remote repository URL into a local destination directory and open it.",
  command: "clone",
  args: [
    {
      name: "url",
      kind: "text",
      label: "Remote URL",
      description: "URL of the remote repository to clone.",
      required: true,
      placeholder: "https://example.com/repo",
    },
    {
      name: "dest",
      kind: "text",
      label: "Destination",
      description: "Local directory to clone into.",
      required: true,
      placeholder: "/path/to/local/repo",
    },
  ],
  resultKind: "void",
  keywords: ["clone", "checkout", "download", "remote", "copy", "url"],
};

export default manifest;
