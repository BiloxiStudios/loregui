import type { OpManifest } from "../../types";

/**
 * Reference manifest entry (Phase 0). A two-arg op with a JSON result —
 * exercises the multi-arg-form + object-result path of the palette.
 */
const manifest: OpManifest = {
  id: "repository.verify_state",
  domain: "repository",
  op: "verify_state",
  label: "Repository: Verify State",
  description:
    "Verify repository integrity; optionally heal detected inconsistencies. " +
    "Returns fragment-level verification events and a typed summary.",
  command: "repository_verify_state",
  args: [
    {
      name: "path",
      kind: "text",
      label: "Path",
      description: "Repository-relative path to verify; empty verifies the whole repository.",
      required: false,
      default: "",
    },
    {
      name: "heal",
      kind: "boolean",
      label: "Heal",
      description: "When true, attempt to heal detected inconsistencies.",
      required: false,
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["verify", "integrity", "heal", "check", "repository", "state", "corruption"],
};

export default manifest;
