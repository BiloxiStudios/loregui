import { useEffect, useRef, useState } from "react";
import { EditorState, type Extension } from "@codemirror/state";
import { EditorView, lineNumbers } from "@codemirror/view";
import { oneDark } from "@codemirror/theme-one-dark";
import { workingFileApi } from "../api";
import { cmLangOf, formatSize, type CmLang } from "./kinds";

/**
 * Read-only syntax-highlighted text/code preview (SBAI-4083). Reuses CodeMirror
 * (already in the bundle for Edit) in read-only mode so Preview and Edit share
 * one highlighter. Lazy-imported by PreviewView so CodeMirror only loads when a
 * text file is actually opened.
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

export default function TextPreview({ path }: { path: string }) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tooLarge, setTooLarge] = useState<number | null>(null);

  useEffect(() => {
    let disposed = false;
    setLoading(true);
    setError(null);
    setTooLarge(null);
    void (async () => {
      try {
        const res = await workingFileApi.readText(path);
        if (disposed) return;
        if (res.too_large) {
          setTooLarge(res.size);
          setLoading(false);
          return;
        }
        const langExt = await langExtension(cmLangOf(path));
        if (disposed || !hostRef.current) return;
        const view = new EditorView({
          state: EditorState.create({
            doc: res.content,
            extensions: [
              lineNumbers(),
              EditorState.readOnly.of(true),
              EditorView.editable.of(false),
              oneDark,
              EditorView.theme({
                "&": { height: "100%", fontSize: "13px" },
                ".cm-scroller": { fontFamily: "var(--font-family, monospace)" },
              }),
              ...langExt,
            ],
          }),
          parent: hostRef.current,
        });
        viewRef.current = view;
        setLoading(false);
      } catch (e) {
        if (!disposed) {
          setError(e instanceof Error ? e.message : String(e));
          setLoading(false);
        }
      }
    })();
    return () => {
      disposed = true;
      viewRef.current?.destroy();
      viewRef.current = null;
    };
  }, [path]);

  if (error)
    return (
      <p className="cw-error" role="alert">
        {error}
      </p>
    );
  if (tooLarge != null)
    return (
      <p className="cw-empty">
        File is {formatSize(tooLarge)} — too large to preview inline.
      </p>
    );
  return (
    <div className="cw-text-preview">
      {loading && <p className="cw-status">Loading preview…</p>}
      <div ref={hostRef} className="cw-cm-host" hidden={loading} />
    </div>
  );
}
