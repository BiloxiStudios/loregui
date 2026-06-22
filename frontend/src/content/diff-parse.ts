// Unified-diff parsing for the content workspace Diff tab (SBAI-4084).
//
// `file_diff` returns one `patch` string per path (standard unified diff). We
// parse it into typed rows for two renderers: an inline unified view (one column,
// +/-/context markers) and a side-by-side view (old | new aligned). Pure and
// tested-via-build; no DOM.

export type RowKind = "context" | "add" | "del" | "hunk" | "meta";

/** One row of the inline unified diff. */
export interface UnifiedRow {
  kind: RowKind;
  /** Old-file line number (null for adds / hunk / meta). */
  oldNo: number | null;
  /** New-file line number (null for dels / hunk / meta). */
  newNo: number | null;
  text: string;
}

/** One aligned row of the side-by-side diff (either side may be empty). */
export interface SideRow {
  kind: "context" | "add" | "del" | "change";
  oldNo: number | null;
  newNo: number | null;
  oldText: string | null;
  newText: string | null;
}

const HUNK_RE = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/;

/** Parse a unified-diff patch string into inline rows. */
export function parseUnified(patch: string): UnifiedRow[] {
  const rows: UnifiedRow[] = [];
  if (!patch) return rows;
  let oldNo = 0;
  let newNo = 0;
  for (const line of patch.split("\n")) {
    if (line.startsWith("@@")) {
      const m = HUNK_RE.exec(line);
      if (m) {
        oldNo = parseInt(m[1], 10);
        newNo = parseInt(m[3], 10);
      }
      rows.push({ kind: "hunk", oldNo: null, newNo: null, text: line });
      continue;
    }
    if (
      line.startsWith("--- ") ||
      line.startsWith("+++ ") ||
      line.startsWith("diff ") ||
      line.startsWith("index ") ||
      line.startsWith("new file") ||
      line.startsWith("deleted file") ||
      line.startsWith("rename ")
    ) {
      rows.push({ kind: "meta", oldNo: null, newNo: null, text: line });
      continue;
    }
    if (line.startsWith("+")) {
      rows.push({ kind: "add", oldNo: null, newNo: newNo, text: line.slice(1) });
      newNo += 1;
    } else if (line.startsWith("-")) {
      rows.push({ kind: "del", oldNo: oldNo, newNo: null, text: line.slice(1) });
      oldNo += 1;
    } else if (line.startsWith("\\")) {
      // "\ No newline at end of file"
      rows.push({ kind: "meta", oldNo: null, newNo: null, text: line });
    } else {
      const text = line.startsWith(" ") ? line.slice(1) : line;
      rows.push({ kind: "context", oldNo, newNo, text });
      oldNo += 1;
      newNo += 1;
    }
  }
  return rows;
}

/** Fold inline rows into aligned side-by-side rows (del+add → change). */
export function toSideBySide(rows: UnifiedRow[]): SideRow[] {
  const out: SideRow[] = [];
  let i = 0;
  while (i < rows.length) {
    const r = rows[i];
    if (r.kind === "hunk" || r.kind === "meta") {
      i += 1;
      continue;
    }
    if (r.kind === "context") {
      out.push({
        kind: "context",
        oldNo: r.oldNo,
        newNo: r.newNo,
        oldText: r.text,
        newText: r.text,
      });
      i += 1;
      continue;
    }
    // Gather a run of dels followed by a run of adds → pair them as changes.
    const dels: UnifiedRow[] = [];
    const adds: UnifiedRow[] = [];
    while (i < rows.length && rows[i].kind === "del") dels.push(rows[i++]);
    while (i < rows.length && rows[i].kind === "add") adds.push(rows[i++]);
    const max = Math.max(dels.length, adds.length);
    for (let k = 0; k < max; k += 1) {
      const d = dels[k];
      const a = adds[k];
      if (d && a) {
        out.push({
          kind: "change",
          oldNo: d.oldNo,
          newNo: a.newNo,
          oldText: d.text,
          newText: a.text,
        });
      } else if (d) {
        out.push({
          kind: "del",
          oldNo: d.oldNo,
          newNo: null,
          oldText: d.text,
          newText: null,
        });
      } else if (a) {
        out.push({
          kind: "add",
          oldNo: null,
          newNo: a.newNo,
          oldText: null,
          newText: a.text,
        });
      }
    }
  }
  return out;
}

/** Add/remove counts for the diff header. */
export function diffStat(rows: UnifiedRow[]): { added: number; removed: number } {
  let added = 0;
  let removed = 0;
  for (const r of rows) {
    if (r.kind === "add") added += 1;
    else if (r.kind === "del") removed += 1;
  }
  return { added, removed };
}
