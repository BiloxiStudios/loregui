import type { OpManifest } from "../../types";

/**
 * Palette manifest for `lock.file_query`.
 *
 * Queries file locks on a branch, optionally filtered by owner and path.
 */
const manifest: OpManifest = {
  id: "lock.file_query",
  domain: "lock",
  op: "file_query",
  label: "Lock: Query",
  description:
    "List file locks on a branch, optionally filtered by owner or path.",
  command: "lock_file_query",
  args: [
    {
      name: "branch",
      kind: "text",
      label: "Branch",
      description: "Branch to query locks on; leave empty for current branch.",
      required: false,
      placeholder: "e.g. main",
    },
    {
      name: "owner",
      kind: "text",
      label: "Owner",
      description: "Filter by lock owner; leave empty for all owners.",
      required: false,
      placeholder: "e.g. user@example.com",
    },
    {
      name: "path",
      kind: "text",
      label: "Path",
      description: "Filter by path prefix; leave empty for all paths.",
      required: false,
      placeholder: "e.g. Content/Maps/",
    },
  ],
  resultKind: "json",
  keywords: ["lock", "query", "list", "search", "find", "who"],
};

export default manifest;
