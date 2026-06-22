import { useEffect, useMemo, useRef, useState } from "react";
import { fileDiffApi } from "../api";
import {
  parseUnified,
  toSideBySide,
  diffStat,
  type UnifiedRow,
  type SideRow,
} from "./diff-parse";

/**
 * Diff tab of the content workspace (SBAI-4084).
 *
 * Drives off `file_diff` (one unified `patch` per path). Renders the file's
 * working-vs-revision diff two ways — inline unified and side-by-side — with
 * per-line add/remove/change markers and line numbers. Large diffs are
 * virtualized (windowed render) so a 50k-line patch stays responsive. Reachable
 * from the Changes file view (working vs staged/committed) and from History
 * (diff a revision) by passing the matching source/target revision.
 *
 * Image before/after (swipe/onion-skin) is intentionally out of scope here — the
 * Preview tab already renders images; binary diffs show a "binary file changed"
 * notice rather than a byte patch.
 */

const ROW_H = 20; // px per diff line (must match CSS .cw-diff-row height)
const OVERSCAN = 12;

export default function DiffView({
  path,
  sourceRevision,
  targetRevision,
}: {
  path: string;
  sourceRevision: string;
  targetRevision: string;
}) {
  const [rows, setRows] = useState<UnifiedRow[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<"unified" | "split">("split");
  const [binary, setBinary] = useState(false);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportH, setViewportH] = useState(480);

  const load = useMemo(
    () => async () => {
      setLoading(true);
      setError(null);
      setBinary(false);
      try {
        const entries = await fileDiffApi.diff(
          [path],
          sourceRevision,
          targetRevision,
        );
        const entry = entries.find((e) => e.path === path) ?? entries[0];
        const patch = entry?.patch ?? "";
        if (/Binary files? .* differ/i.test(patch)) {
          setBinary(true);
          setRows([]);
        } else {
          setRows(parseUnified(patch));
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setLoading(false);
      }
    },
    [path, sourceRevision, targetRevision],
  );

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const onResize = () => setViewportH(el.clientHeight || 480);
    onResize();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [rows, mode]);

  const split: SideRow[] = useMemo(
    () => (rows ? toSideBySide(rows) : []),
    [rows],
  );
  const stat = useMemo(() => (rows ? diffStat(rows) : { added: 0, removed: 0 }), [rows]);

  const list: Array<UnifiedRow | SideRow> = mode === "unified" ? rows ?? [] : split;
  const total = list.length;
  const first = Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN);
  const visibleCount = Math.ceil(viewportH / ROW_H) + OVERSCAN * 2;
  const last = Math.min(total, first + visibleCount);
  const padTop = first * ROW_H;
  const padBottom = (total - last) * ROW_H;

  if (loading) return <p className="cw-status">Loading diff…</p>;
  if (error)
    return (
      <div className="cw-error" role="alert">
        <p>{error}</p>
        <button onClick={() => void load()}>Retry</button>
      </div>
    );
  if (binary)
    return (
      <p className="cw-empty">
        Binary file changed — no text diff. Use Preview to inspect it.
      </p>
    );
  if (total === 0)
    return <p className="cw-empty">No changes for this file.</p>;

  return (
    <div className="cw-diff">
      <div className="cw-diff-bar">
        <span className="cw-diff-stat">
          <span className="cw-add">+{stat.added}</span>{" "}
          <span className="cw-del">−{stat.removed}</span>
        </span>
        <div className="cw-seg" role="tablist" aria-label="Diff layout">
          <button
            role="tab"
            aria-selected={mode === "split"}
            className={mode === "split" ? "cw-seg-on" : ""}
            onClick={() => setMode("split")}
          >
            Side-by-side
          </button>
          <button
            role="tab"
            aria-selected={mode === "unified"}
            className={mode === "unified" ? "cw-seg-on" : ""}
            onClick={() => setMode("unified")}
          >
            Unified
          </button>
        </div>
      </div>
      <div
        ref={scrollRef}
        className="cw-diff-scroll"
        onScroll={(e) => setScrollTop((e.target as HTMLDivElement).scrollTop)}
      >
        <div style={{ paddingTop: padTop, paddingBottom: padBottom }}>
          {mode === "unified"
            ? (list.slice(first, last) as UnifiedRow[]).map((r, idx) => (
                <UnifiedLine key={first + idx} row={r} />
              ))
            : (list.slice(first, last) as SideRow[]).map((r, idx) => (
                <SplitLine key={first + idx} row={r} />
              ))}
        </div>
      </div>
    </div>
  );
}

function UnifiedLine({ row }: { row: UnifiedRow }) {
  const sign =
    row.kind === "add" ? "+" : row.kind === "del" ? "−" : row.kind === "context" ? " " : "";
  return (
    <div className={`cw-diff-row cw-u-${row.kind}`}>
      <span className="cw-ln">{row.oldNo ?? ""}</span>
      <span className="cw-ln">{row.newNo ?? ""}</span>
      <span className="cw-sign" aria-hidden="true">
        {sign}
      </span>
      <span className="cw-code">{row.text}</span>
    </div>
  );
}

function SplitLine({ row }: { row: SideRow }) {
  const oldCls =
    row.kind === "del" || row.kind === "change" ? "cw-s-del" : "cw-s-ctx";
  const newCls =
    row.kind === "add" || row.kind === "change" ? "cw-s-add" : "cw-s-ctx";
  return (
    <div className="cw-diff-row cw-split">
      <span className="cw-ln">{row.oldNo ?? ""}</span>
      <span className={`cw-code cw-half ${row.oldText == null ? "cw-empty-half" : oldCls}`}>
        {row.oldText ?? ""}
      </span>
      <span className="cw-ln">{row.newNo ?? ""}</span>
      <span className={`cw-code cw-half ${row.newText == null ? "cw-empty-half" : newCls}`}>
        {row.newText ?? ""}
      </span>
    </div>
  );
}
