import type { OpManifest } from "../../types";

/**
 * Shared-store create manifest.
 *
 * Creates a new shared store at the specified path, optionally setting it as
 * the default. Remote URL and path are required; make_default defaults to false.
 */
const manifest: OpManifest = {
  id: "shared_store.create",
  domain: "shared_store",
  op: "create",
  label: "Shared Store: Create",
  description:
    "Create a new shared store at the specified path, optionally setting it as the default.",
  command: "shared_store_create",
  args: [
    {
      name: "remoteUrl",
      kind: "text",
      label: "Remote URL",
      description: "Remote URL backing the store (leave empty for local store).",
      required: false,
      default: "",
      placeholder: "https://example.com/repo",
    },
    {
      name: "path",
      kind: "text",
      label: "Path",
      description: "Filesystem path where the store will be created (empty for default location).",
      required: false,
      default: "",
      placeholder: "/home/user/.lore/shared-store",
    },
    {
      name: "makeDefault",
      kind: "boolean",
      label: "Make Default",
      description: "Set this as the default shared store in global config.",
      required: false,
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["shared", "store", "create", "new", "init"],
};

export default manifest;
