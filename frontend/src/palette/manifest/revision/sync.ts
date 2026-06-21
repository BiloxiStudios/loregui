import type { OpManifest } from "../../types";

/** Sync the working tree to a target revision, including dependencies. */
const manifest: OpManifest = {
  id: "revision.sync",
  domain: "revision",
  op: "sync",
  label: "Revision: Sync",
  description:
    "Sync the working tree to a target revision, optionally resetting and pulling dependencies.",
  command: "revision_sync",
  args: [
    {
      name: "revision",
      kind: "text",
      label: "Revision",
      description: "Target revision to sync to; empty syncs to the latest.",
      required: false,
      default: "",
      placeholder: "e.g. abc123",
    },
    {
      name: "forwardChanges",
      kind: "boolean",
      label: "Forward changes",
      description: "Carry local changes forward across the sync.",
      required: false,
      default: false,
    },
    {
      name: "reset",
      kind: "boolean",
      label: "Reset",
      description: "Discard local changes and reset to the target revision.",
      required: false,
      default: false,
    },
    {
      name: "rootFiles",
      kind: "string-list",
      label: "Root files",
      description: "Restrict the sync to these root files (one per line).",
      required: false,
    },
    {
      name: "dependencyTags",
      kind: "string-list",
      label: "Dependency tags",
      description: "Dependency tags to include when syncing (one per line).",
      required: false,
    },
    {
      name: "dependencyRecursive",
      kind: "boolean",
      label: "Recursive dependencies",
      description: "Resolve dependencies recursively.",
      required: false,
      default: false,
    },
    {
      name: "dependencyDepthLimit",
      kind: "number",
      label: "Dependency depth limit",
      description: "Maximum depth when resolving dependencies (0 = unlimited).",
      required: false,
      default: 0,
    },
  ],
  resultKind: "json",
  keywords: ["revision", "sync", "checkout", "update", "dependencies", "pull"],
};

export default manifest;
