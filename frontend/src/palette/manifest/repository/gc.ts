import type { OpManifest } from "../../types";

/**
 * Palette manifest for `repository.gc`.
 *
 * Runs garbage collection on the local repository store to reclaim space from
 * unreferenced data. No arguments required — invokes `repository_gc` and
 * displays any diagnostic log messages from the GC run.
 */
const manifest: OpManifest = {
  id: "repository.gc",
  domain: "repository",
  op: "gc",
  label: "Repository: Garbage Collect",
  description:
    "Run garbage collection on the local repository store to reclaim space.",
  command: "repository_gc",
  args: [],
  resultKind: "json",
  keywords: [
    "gc",
    "garbage",
    "collect",
    "clean",
    "reclaim",
    "space",
    "prune",
    "housekeeping",
  ],
};

export default manifest;
