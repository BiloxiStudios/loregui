import type { OpManifest } from "../types";

/**
 * The command-palette op registry.
 *
 * Entries are auto-discovered from `manifest/<domain>/<op>.ts` via Vite's
 * `import.meta.glob` — so adding an op is **one new file, zero shared-file
 * edits** (no index to append, no merge conflicts during the parity fan-out).
 * Each entry module must `export default` an {@link OpManifest}.
 *
 * Do NOT convert this back to manual imports — that reintroduces the merge
 * contention the glob removes (see CLAUDE.md).
 */
const modules = import.meta.glob<{ default: OpManifest }>("./*/*.ts", {
  eager: true,
});

export const OP_MANIFEST: OpManifest[] = Object.values(modules)
  .map((m) => m.default)
  .filter((m): m is OpManifest => Boolean(m && m.id))
  .sort((a, b) => a.id.localeCompare(b.id));

/** Lookup by `"<domain>.<op>"` id. */
export const OP_BY_ID: Record<string, OpManifest> = Object.fromEntries(
  OP_MANIFEST.map((m) => [m.id, m]),
);
