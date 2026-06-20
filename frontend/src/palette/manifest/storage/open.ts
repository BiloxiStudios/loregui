import type { OpManifest } from "../../types";

/**
 * Storage open manifest. Acquires a handle to a content-addressed store,
 * either disk-backed or fully in-memory. Returns an opaque handle ID for
 * subsequent storage operations.
 */
const manifest: OpManifest = {
  id: "storage.open",
  domain: "storage",
  op: "open",
  label: "Storage: Open",
  description:
    "Open a content-addressed store (disk-backed or in-memory) and return its handle for subsequent operations.",
  command: "storage_open",
  args: [
    {
      name: "repositoryPath",
      kind: "text",
      label: "Repository Path",
      description: "Path to an existing lore repository (empty when inMemory is true).",
      required: false,
      default: "",
    },
    {
      name: "inMemory",
      kind: "boolean",
      label: "In-Memory",
      description: "Open a fresh in-memory store (repositoryPath must be empty when set).",
      required: false,
      default: false,
    },
    {
      name: "remoteUrl",
      kind: "text",
      label: "Remote URL",
      description: "Optional remote endpoint URL for ops that consult a peer service.",
      required: false,
      default: "",
    },
    {
      name: "cacheTargetBytes",
      kind: "number",
      label: "Cache Target Bytes",
      description: "Soft cap on total immutable-store bytes (0 selects default).",
      required: false,
      default: 0,
    },
    {
      name: "cacheTargetFragments",
      kind: "number",
      label: "Cache Target Fragments",
      description: "Soft cap on immutable-store fragment count (0 selects default).",
      required: false,
      default: 0,
    },
  ],
  resultKind: "json",
  keywords: ["open", "store", "storage", "handle", "repository"],
};

export default manifest;
