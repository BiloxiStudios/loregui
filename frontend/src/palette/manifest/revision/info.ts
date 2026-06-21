import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for revision.info.
 *
 * Retrieves metadata and file-change information for a revision.
 * Optionally includes per-file deltas and key/value metadata entries.
 */
const manifest: OpManifest = {
  id: "revision.info",
  domain: "revision",
  op: "info",
  label: "Revision: Info",
  description: "Show metadata and file changes for a revision.",
  command: "revision_info",
  args: [
    {
      name: "revision",
      kind: "text",
      label: "Revision",
      description: "Revision to inspect; empty for current.",
      required: false,
      placeholder: "e.g. abc123def",
    },
    {
      name: "delta",
      kind: "boolean",
      label: "Include Delta",
      description: "Include per-file changes against the parent revision.",
      required: false,
      default: false,
    },
    {
      name: "metadata",
      kind: "boolean",
      label: "Include Metadata",
      description: "Include key/value metadata entries.",
      required: false,
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["info", "details", "inspect", "revision", "metadata", "delta"],
};

export default manifest;
