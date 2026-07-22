import type { OpManifest } from "../../types";

/**
 * Open a content-addressed store and return its handle. The handle threads into
 * every other storage op (close/flush/get_metadata/copy/upload/put_file).
 */
const manifest: OpManifest = {
  id: "storage.open",
  domain: "storage",
  op: "open",
  label: "Storage: Open",
  description:
    "Open a content-addressed store (local path, remote URL, or in-memory) and return its handle.",
  command: "storage_open_handle",
  requiresRepository: false,
  args: [
    {
      name: "repositoryPath",
      kind: "text",
      label: "Repository path",
      description: "Local lore repository to open. Leave empty for remote/in-memory.",
      placeholder: "/path/to/lore/repo",
    },
    {
      name: "remoteUrl",
      kind: "text",
      label: "Remote URL",
      description: "Remote store endpoint. Leave empty for a local-only store.",
      placeholder: "https://store.example.com",
    },
    {
      name: "inMemory",
      kind: "boolean",
      label: "In-memory store",
      description: "Open a throwaway in-memory store (path and remote URL must be empty).",
    },
  ],
  resultKind: "json",
  keywords: ["storage", "open", "handle", "store", "connect"],
  surface: "panel",
};

export default manifest;
