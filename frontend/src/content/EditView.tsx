import { useCallback, useEffect, useRef, useState } from "react";
import { EditorState, type Extension } from "@codemirror/state";
import { EditorView, keymap, lineNumbers, gutter, GutterMarker } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { oneDark } from "@codemirror/theme-one-dark";
import { workingFileApi, fileStageApi, type ChangeKind } from "../api";
import { cmLangOf, type CmLang } from "./kinds";

/**
 * CodeMirror 6 editor for the content workspace Edit tab (SBAI-4085).
 *
 * Text/code only — line numbers, syntax highlight, and a lore change-status
 * gutter marking the file's working-tree state (added/modified/etc). Save writes
 * the working tree via `write_text_file`; "Stage" runs `file_stage` so the user
 * can stage straight from the editor. Binary/large files never reach here (the
 * workspace gates Edit to text under the size cap) and render read-only.
 *
 * CodeMirror is dynamically imported by the workspace's lazy boundary, so its
 * ~250 KB ships in a separate chunk fetched only when the Edit tab opens.
 */

async function langExtension(lang: CmLang): Promise<Extension[]> {
  switch (lang) {
    case "javascript":
    case "typescript": {
      const { javascript } = await import("@codemirror/lang-javascript");
      return [javascript({ typescript: lang === "typescript", jsx: true })];
    }
    case "json": {
      const { json } = await import("@codemirror/lang-json");
      return [json()];
    }
    case "markdown": {
      const { markdown } = await import("@codemirror/lang-markdown");
      return [markdown()];
    }
    case "rust": {
      const { rust } = await import("@codemirror/lang-rust");
      return [rust()];
    }
    default:
      return [];
  }
}

/** A gutter marker that paints the file's lore change status on every line. */
class StatusMarker extends GutterMarker {
  constructor(private readonly kind: ChangeKind) {
    super();
  }
  override toDOM() {
    const span = document.createElement("span");
    span.className = `cw-cm-status cw-cm-status-${this.kind}`;
    span.title = `working-tree status: ${this.kind}`;
    return span;
  }
}

function statusGutter(kind: ChangeKind | null): Extension {
  if (!kind) return [];
  const marker = new StatusMarker(kind);
  return gutter({
    class: "cw-cm-status-gutter",
    lineMarker: () => marker,
    initialSpacer: () => marker,
  });
}

export default function EditView({
  path,
  changeKind,
  readOnly,
  onSaved,
  onStaged,
  onError,
}: {
  path: string;
  changeKind: ChangeKind | null;
  readOnly: boolean;
  onSaved: () => void;
  onStaged: () => void;
  onError: (msg: string) => void;
}) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  const [loading, setLoading] = useState(true);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [staging, setStaging] = useState(false);

  useEffect(() => {
    let disposed = false;
    setLoading(true);
    setDirty(false);

    void (async () => {
      try {
        const res = await workingFileApi.readText(path);
        if (disposed) return;
        const langExt = await langExtension(cmLangOf(path));
        if (disposed || !hostRef.current) return;

        const extensions: Extension[] = [
          lineNumbers(),
          history(),
          statusGutter(changeKind),
          keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
          oneDark,
          EditorView.theme({
            "&": { height: "100%", fontSize: "13px" },
            ".cm-scroller": { fontFamily: "var(--font-family, monospace)" },
          }),
          ...langExt,
          EditorView.editable.of(!readOnly),
          EditorState.readOnly.of(readOnly),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) setDirty(true);
          }),
        ];

        const view = new EditorView({
          state: EditorState.create({ doc: res.content, extensions }),
          parent: hostRef.current,
        });
        viewRef.current = view;
        setLoading(false);
      } catch (e) {
        if (!disposed) {
          setLoading(false);
          onError(e instanceof Error ? e.message : String(e));
        }
      }
    })();

    return () => {
      disposed = true;
      viewRef.current?.destroy();
      viewRef.current = null;
    };
    // changeKind/readOnly are stable per open; re-mount on path change only.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path]);

  const save = useCallback(async () => {
    const view = viewRef.current;
    if (!view) return;
    setSaving(true);
    try {
      await workingFileApi.writeText(path, view.state.doc.toString());
      setDirty(false);
      onSaved();
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }, [path, onSaved, onError]);

  const saveAndStage = useCallback(async () => {
    const view = viewRef.current;
    if (!view) return;
    setStaging(true);
    try {
      if (dirty) {
        await workingFileApi.writeText(path, view.state.doc.toString());
        setDirty(false);
        onSaved();
      }
      await fileStageApi.stage([path]);
      onStaged();
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setStaging(false);
    }
  }, [path, dirty, onSaved, onStaged, onError]);

  // Ctrl/Cmd-S saves.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        if (!readOnly) void save();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [save, readOnly]);

  return (
    <div className="cw-edit">
      <div className="cw-edit-bar">
        <span className="cw-edit-state">
          {readOnly
            ? "read-only"
            : dirty
              ? "unsaved changes"
              : "saved"}
        </span>
        <div className="cw-edit-actions">
          <button
            onClick={() => void saveAndStage()}
            disabled={readOnly || saving || staging}
            title="Save then stage this file"
          >
            {staging ? "Staging…" : "Save & stage"}
          </button>
          <button
            className="cw-primary"
            onClick={() => void save()}
            disabled={readOnly || !dirty || saving || staging}
            title="Write changes to the working tree (Ctrl/Cmd-S)"
          >
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </div>
      {loading && <p className="cw-status">Loading file…</p>}
      <div ref={hostRef} className="cw-cm-host" hidden={loading} />
    </div>
  );
}
