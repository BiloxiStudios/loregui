import type { OpManifest } from "../../types";

/**
 * Manifest entry for revision.commit_with_metadata.
 *
 * Commits staged changes as a new revision with attached metadata key-value
 * pairs. Each metadata entry has a key, value, and format type (Binary,
 * Numeric, or String). The parallel arrays keys/values/formats must have
 * matching lengths — one entry per index.
 *
 * Use metadata_get to read keys and metadata_set to write them on existing
 * revisions; commit_with_metadata attaches them atomically at commit time.
 */
const manifest: OpManifest = {
  id: "revision.commit_with_metadata",
  domain: "revision",
  op: "commit_with_metadata",
  label: "Revision: Commit with Metadata",
  description: "Commit staged changes with attached metadata key-value pairs.",
  command: "commit_with_metadata",
  args: [
    {
      name: "message",
      kind: "text",
      label: "Message",
      description: "Commit message describing the revision.",
      required: true,
      placeholder: "Describe the change",
    },
    {
      name: "keys",
      kind: "string-list",
      label: "Keys",
      description: "One metadata key per line. Must match values/formats length.",
      required: false,
      placeholder: "author\npriority\nticket",
    },
    {
      name: "values",
      kind: "string-list",
      label: "Values",
      description: "One value per line. Must match keys/formats length.",
      required: false,
      placeholder: "alice\n42\nSBAI-3911",
    },
    {
      name: "formats",
      kind: "string-list",
      label: "Format Types",
      description: "One format per line (binary, numeric, or string). Must match keys/values length.",
      required: false,
      placeholder: "string\nstring\nstring",
    },
  ],
  resultKind: "json",
  keywords: ["commit", "metadata", "tag", "annotate"],
};

export default manifest;
