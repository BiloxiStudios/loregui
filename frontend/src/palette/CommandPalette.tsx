import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { invoke } from "@tauri-apps/api/core";
import { OP_MANIFEST } from "./manifest";
import type { OpManifest } from "./types";
import OpForm from "./form";
import OpResult from "./result";

/** Window event the topbar launcher dispatches to open the palette. */
export const OPEN_PALETTE_EVENT = "loregui:open-palette";

function matches(m: OpManifest, q: string): boolean {
  if (!q) return true;
  const hay = `${m.id} ${m.label} ${m.description ?? ""} ${(m.keywords ?? []).join(" ")}`.toLowerCase();
  return q
    .toLowerCase()
    .split(/\s+/)
    .every((term) => hay.includes(term));
}

const overlayStyle: CSSProperties = {
  position: "fixed",
  inset: 0,
  background: "rgba(0,0,0,0.5)",
  display: "flex",
  alignItems: "flex-start",
  justifyContent: "center",
  padding: "10vh 16px 16px",
  zIndex: 1100,
};
const panelStyle: CSSProperties = {
  width: "100%",
  maxWidth: 640,
  maxHeight: "75vh",
  display: "flex",
  flexDirection: "column",
  background: "var(--surface-overlay-bg)",
  color: "var(--surface-base-text)",
  border: "1px solid var(--surface-overlay-border)",
  borderRadius: 8,
  boxShadow: "var(--shadow-lg)",
  overflow: "hidden",
};
const searchStyle: CSSProperties = {
  width: "100%",
  boxSizing: "border-box",
  padding: "12px 14px",
  background: "transparent",
  color: "var(--surface-base-text)",
  border: "none",
  borderBottom: "1px solid var(--surface-overlay-border)",
  fontSize: 15,
  outline: "none",
};

export default function CommandPalette() {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<OpManifest | null>(null);
  const [highlight, setHighlight] = useState(0);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<{ value: unknown } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const reset = useCallback(() => {
    setQuery("");
    setSelected(null);
    setHighlight(0);
    setResult(null);
    setError(null);
    setRunning(false);
  }, []);

  const close = useCallback(() => {
    setOpen(false);
    reset();
  }, [reset]);

  const filtered = useMemo(
    () =>
      OP_MANIFEST.filter((m) => matches(m, query)).sort((a, b) =>
        a.label.localeCompare(b.label),
      ),
    [query],
  );

  // Global open shortcut (Ctrl/Cmd-K) + launcher event.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((o) => !o);
      }
    };
    const onOpen = () => setOpen(true);
    window.addEventListener("keydown", onKey);
    window.addEventListener(OPEN_PALETTE_EVENT, onOpen);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener(OPEN_PALETTE_EVENT, onOpen);
    };
  }, []);

  useEffect(() => {
    if (open && !selected) searchRef.current?.focus();
  }, [open, selected]);

  useEffect(() => {
    if (highlight >= filtered.length) setHighlight(Math.max(0, filtered.length - 1));
  }, [filtered, highlight]);

  const pick = useCallback((m: OpManifest) => {
    setSelected(m);
    setResult(null);
    setError(null);
  }, []);

  const run = useCallback(
    async (args: Record<string, unknown>) => {
      if (!selected) return;
      setRunning(true);
      setError(null);
      setResult(null);
      try {
        const value = await invoke(selected.command, args);
        setResult({ value });
      } catch (e) {
        setError(typeof e === "string" ? e : JSON.stringify(e));
      } finally {
        setRunning(false);
      }
    },
    [selected],
  );

  if (!open) return null;

  const onListKey = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setHighlight((h) => Math.min(h + 1, filtered.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setHighlight((h) => Math.max(h - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const m = filtered[highlight];
      if (m) pick(m);
    } else if (e.key === "Escape") {
      e.preventDefault();
      close();
    }
  };

  return (
    <div style={overlayStyle} onClick={close}>
      <div
        style={panelStyle}
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
      >
        {!selected ? (
          <>
            <input
              ref={searchRef}
              style={searchStyle}
              placeholder="Run a command…  (type to search all ops)"
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setHighlight(0);
              }}
              onKeyDown={onListKey}
            />
            <div style={{ overflowY: "auto" }}>
              {filtered.length === 0 && (
                <div
                  style={{
                    padding: 16,
                    color: "var(--surface-base-text-secondary)",
                    fontSize: 13,
                  }}
                >
                  No commands match “{query}”.
                </div>
              )}
              {filtered.map((m, i) => (
                <button
                  key={m.id}
                  onClick={() => pick(m)}
                  onMouseEnter={() => setHighlight(i)}
                  style={{
                    display: "block",
                    width: "100%",
                    textAlign: "left",
                    padding: "9px 14px",
                    border: "none",
                    cursor: "pointer",
                    background:
                      i === highlight
                        ? "var(--surface-primary-bg)"
                        : "transparent",
                    color:
                      i === highlight
                        ? "var(--surface-primary-text)"
                        : "var(--surface-base-text)",
                  }}
                >
                  <div style={{ fontSize: 13, fontWeight: 600 }}>{m.label}</div>
                  {m.description && (
                    <div
                      style={{
                        fontSize: 11,
                        opacity: 0.8,
                        color: "inherit",
                      }}
                    >
                      {m.description}
                    </div>
                  )}
                </button>
              ))}
            </div>
            <div
              style={{
                padding: "6px 14px",
                fontSize: 11,
                color: "var(--surface-base-text-secondary)",
                borderTop: "1px solid var(--surface-overlay-border)",
              }}
            >
              {filtered.length} command{filtered.length === 1 ? "" : "s"} · ↑↓ to
              navigate · Enter to select · Esc to close
            </div>
          </>
        ) : (
          <div style={{ padding: 16, overflowY: "auto" }}>
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                marginBottom: 12,
              }}
            >
              <div>
                <div style={{ fontSize: 14, fontWeight: 700 }}>
                  {selected.label}
                </div>
                <code
                  style={{
                    fontSize: 11,
                    color: "var(--surface-base-text-secondary)",
                  }}
                >
                  {selected.command}
                </code>
              </div>
              <button
                onClick={() => {
                  setSelected(null);
                  setResult(null);
                  setError(null);
                }}
              >
                ← Back
              </button>
            </div>
            {selected.description && (
              <p
                style={{
                  fontSize: 12,
                  color: "var(--surface-base-text-secondary)",
                  margin: "0 0 14px",
                }}
              >
                {selected.description}
              </p>
            )}
            <OpForm manifest={selected} running={running} onRun={run} />
            {error && (
              <div
                style={{
                  marginTop: 12,
                  padding: "10px 12px",
                  background: "var(--surface-error-bg)",
                  color: "var(--surface-error-text)",
                  border: "1px solid var(--surface-error-border)",
                  borderRadius: 4,
                  fontSize: 12,
                  whiteSpace: "pre-wrap",
                }}
              >
                {error}
              </div>
            )}
            {result && <OpResult value={result.value} kind={selected.resultKind} />}
          </div>
        )}
      </div>
    </div>
  );
}
