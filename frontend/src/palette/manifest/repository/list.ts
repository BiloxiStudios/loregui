import type { OpManifest } from "../../types";

/**
 * Reference manifest entry (Phase 0). A single-arg op whose result is a typed
 * JSON object — exercises the single-field-form + JSON-result path of the palette.
 */
const manifest: OpManifest = {
  id: "repository.list",
  domain: "repository",
  op: "list",
  label: "Repository: List",
  description: "List all repositories available at a remote URL.",
  command: "repository_list",
  args: [
    {
      name: "url",
      kind: "text",
      label: "Remote URL",
      description: "The lore:// URL to query for available repositories.",
      required: true,
      placeholder: "lore://example.com/myrepo",
    },
  ],
  resultKind: "json",
  keywords: ["list", "repositories", "remote", "discover"],
};

export default manifest;
