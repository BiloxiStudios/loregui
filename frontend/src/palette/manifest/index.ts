import type { OpManifest } from "../types";
import repositoryStatus from "./repository/status";
import fileStage from "./file/stage";
import revisionCommit from "./revision/commit";
import linkListStaged from "./link/list_staged";

/**
 * The command-palette op registry.
 *
 * Phase 0 ships three reference entries. The per-op parity fan-out appends one
 * `import` + one array element per op (append-only — the only shared file, the
 * integration manager merges these in order). Keep the array sorted by `id`.
 */
export const OP_MANIFEST: OpManifest[] = [
  fileStage,
  linkListStaged,
  repositoryStatus,
  revisionCommit,
];

/** Lookup by `"<domain>.<op>"` id. */
export const OP_BY_ID: Record<string, OpManifest> = Object.fromEntries(
  OP_MANIFEST.map((m) => [m.id, m]),
);
