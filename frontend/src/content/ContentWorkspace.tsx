import { lazy, Suspense, useEffect, useState } from "react";
import PreviewView from "./PreviewView";
import { kindOf } from "./kinds";
import LockRequestButton from "../locks/LockRequestButton";
import type { ChangeKind } from "../api";

const DiffView = lazy(() => import("./DiffView"));
const EditView = lazy(() => import("./EditView"));

/**
 * Content workspace (SBAI-4083 + 4084 + 4085) — the unified file-detail surface.
 *
 * One overlay panel opened from the Changes/file view (or History) with a
 * segmented Preview | Diff | Edit control over a single working-tree file. The
 * three modes share one header (path + status + close) so selecting a file gives
 * a cohesive "look at it / compare it / change it" surface instead of three
 * disconnected panels:
 *   - Preview: media + syntax-highlighted text (SBAI-4083).
 *   - Diff: working-vs-revision unified/side-by-side diff (SBAI-4084).
 *   - Edit: CodeMirror editor → save working tree + stage (SBAI-4085).
 *
 * Diff/Edit are lazy so their heavy deps (CodeMirror) load on demand. Binary or
 * over-cap files fall back to read-only (Edit is disabled; Preview shows
 * metadata). Themed entirely via `--surface-*`; Esc closes; one primary action.
 */

export type WorkspaceMode = "preview" | "diff" | "edit";

const TABS: { id: WorkspaceMode; label: string }[] = [
  { id: "preview", label: "Preview" },
  { id: "diff", label: "Diff" },
  { id: "edit", label: "Edit" },
];

export default function ContentWorkspace({
  path,
  branch = "",
  changeKind = null,
  initialMode = "preview",
  sourceRevision = "",
  targetRevision = "",
  onStaged,
  onSaved,
  onClose,
}: {
  path: string;
  /** Branch the file's lock is on; used by the "Request check-in" action. */
  branch?: string;
  changeKind?: ChangeKind | null;
  initialMode?: WorkspaceMode;
  /** Diff source revision (empty = working tree). */
  sourceRevision?: string;
  /** Diff target revision (empty = working tree). */
  targetRevision?: string;
  onStaged?: () => void;
  onSaved?: () => void;
  onClose: () => void;
}) {
  const [mode, setMode] = useState<WorkspaceMode>(initialMode);
  const [error, setError] = useState<string | null>(null);
  const [savedTick, setSavedTick] = useState(0);

  const kind = kindOf(path);
  // Edit is text-only; binary/media files open read-only so users can't
  // corrupt them by saving UTF-8 over bytes.
  const editReadOnly = kind !== "text";
  const fileName = path.split(/[\\/]/).pop() ?? path;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      className="cw-scrim"
      role="dialog"
      aria-modal="true"
      aria-label={`Content workspace: ${fileName}`}
      onClick={onClose}
    >
      <div className="cw-panel" onClick={(e) => e.stopPropagation()}>
        <header className="cw-header">
          <div className="cw-title">
            <strong className="cw-name">{fileName}</strong>
            {changeKind && (
              <span className={`cw-badge cw-badge-${changeKind}`}>
                {changeKind}
              </span>
            )}
            <span className="cw-path" title={path}>
              {path}
            </span>
          </div>
          {/* If this file is locked by someone else, offer to ask them to
              check it in (SBAI-4044). Renders nothing otherwise. */}
          <LockRequestButton path={path} branch={branch} className="cw-lock-req" />
          <button className="cw-close" onClick={onClose} title="Close (Esc)">
            Close
          </button>
        </header>

        <div className="cw-tabs" role="tablist" aria-label="File view mode">
          {TABS.map((t) => (
            <button
              key={t.id}
              role="tab"
              aria-selected={mode === t.id}
              className={`cw-tab ${mode === t.id ? "cw-tab-on" : ""}`}
              onClick={() => {
                setError(null);
                setMode(t.id);
              }}
            >
              {t.label}
            </button>
          ))}
        </div>

        {error && (
          <div className="cw-error cw-error-bar" role="alert">
            {error}
          </div>
        )}

        <div className="cw-body">
          {mode === "preview" && <PreviewView path={path} key={`p-${savedTick}`} />}
          {mode === "diff" && (
            <Suspense fallback={<p className="cw-status">Loading diff…</p>}>
              <DiffView
                path={path}
                sourceRevision={sourceRevision}
                targetRevision={targetRevision}
                key={`d-${savedTick}`}
              />
            </Suspense>
          )}
          {mode === "edit" && (
            <Suspense fallback={<p className="cw-status">Loading editor…</p>}>
              <EditView
                path={path}
                changeKind={changeKind}
                readOnly={editReadOnly}
                onSaved={() => {
                  setSavedTick((n) => n + 1);
                  onSaved?.();
                }}
                onStaged={() => onStaged?.()}
                onError={setError}
              />
            </Suspense>
          )}
        </div>
      </div>
    </div>
  );
}
