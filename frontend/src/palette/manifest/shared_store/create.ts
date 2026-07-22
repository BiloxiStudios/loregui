import type { OpManifest } from "../../types";

/** Create a shared store on disk (host setup creates one before repositories). */
const manifest: OpManifest = {
  id: "shared_store.create",
  domain: "shared_store",
  op: "create",
  label: "Shared Store: Create",
  description:
    "Create a shared store on disk that repositories can use as a common content cache.",
  command: "shared_store_create",
  requiresRepository: false,
  args: [
    {
      name: "path",
      kind: "text",
      label: "Store path",
      description: "Filesystem path where the shared store is created.",
      required: true,
      placeholder: "/path/to/shared/store",
    },
  ],
  resultKind: "text",
  keywords: ["shared", "store", "create", "host", "cache"],
  surface: "panel",
};

export default manifest;
