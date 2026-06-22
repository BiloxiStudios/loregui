import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { workingFileApi } from "../api";
import { kindOf, extOf, base64ToBlob, formatSize, type ContentKind } from "./kinds";

const ModelViewer = lazy(() => import("./ModelViewer"));
const TextPreview = lazy(() => import("./TextPreview"));

/**
 * Preview tab of the content workspace (SBAI-4083, core / MIT).
 *
 * Renders the selected working-tree file by kind: images inline; glTF/glb (and
 * best-effort fbx/obj) in a lazy three.js WebGL viewer; audio/video in a native
 * player; text/code syntax-highlighted (read-only CodeMirror, lazy); everything
 * else falls back to file metadata (size, kind, why it can't render). Bytes come
 * from `read_file_bytes` (base64 → Blob URL); text comes from `read_text_file`.
 * Files over the read cap show metadata only.
 */

const MODEL_EXTS = new Set(["gltf", "glb", "fbx", "obj"]);

export default function PreviewView({ path }: { path: string }) {
  const kind: ContentKind = kindOf(path);

  if (kind === "text") return <TextPreviewBoundary path={path} />;
  if (kind === "image") return <MediaPreview path={path} as="image" />;
  if (kind === "audio") return <MediaPreview path={path} as="audio" />;
  if (kind === "video") return <MediaPreview path={path} as="video" />;
  if (kind === "model") return <ModelPreview path={path} />;
  return <BinaryFallback path={path} />;
}

function TextPreviewBoundary({ path }: { path: string }) {
  return (
    <Suspense fallback={<p className="cw-status">Loading preview…</p>}>
      <TextPreview path={path} />
    </Suspense>
  );
}

/** Image / audio / video via an object URL from base64 bytes. */
function MediaPreview({
  path,
  as,
}: {
  path: string;
  as: "image" | "audio" | "video";
}) {
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [meta, setMeta] = useState<{ size: number; tooLarge: boolean } | null>(
    null,
  );
  const urlRef = useRef<string | null>(null);

  useEffect(() => {
    let disposed = false;
    setUrl(null);
    setError(null);
    void (async () => {
      try {
        const res = await workingFileApi.readBytes(path);
        if (disposed) return;
        setMeta({ size: res.size, tooLarge: res.too_large });
        if (res.too_large) return;
        const blob = base64ToBlob(res.base64, res.mime);
        const objUrl = URL.createObjectURL(blob);
        urlRef.current = objUrl;
        setUrl(objUrl);
      } catch (e) {
        if (!disposed) setError(e instanceof Error ? e.message : String(e));
      }
    })();
    return () => {
      disposed = true;
      if (urlRef.current) {
        URL.revokeObjectURL(urlRef.current);
        urlRef.current = null;
      }
    };
  }, [path]);

  if (error)
    return (
      <p className="cw-error" role="alert">
        {error}
      </p>
    );
  if (meta?.tooLarge)
    return (
      <p className="cw-empty">
        File is {formatSize(meta.size)} — too large to preview inline.
      </p>
    );
  if (!url) return <p className="cw-status">Loading preview…</p>;

  if (as === "image")
    return (
      <div className="cw-media cw-media-image">
        <img src={url} alt={path} />
      </div>
    );
  if (as === "audio")
    return (
      <div className="cw-media cw-media-audio">
        <audio src={url} controls />
      </div>
    );
  return (
    <div className="cw-media cw-media-video">
      {/* eslint-disable-next-line jsx-a11y/media-has-caption */}
      <video src={url} controls />
    </div>
  );
}

/** 3D model: load bytes → Blob URL → lazy three.js viewer. */
function ModelPreview({ path }: { path: string }) {
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tooLarge, setTooLarge] = useState(false);
  const [size, setSize] = useState(0);
  const urlRef = useRef<string | null>(null);
  const ext = extOf(path) as "gltf" | "glb" | "fbx" | "obj";

  useEffect(() => {
    let disposed = false;
    setUrl(null);
    setError(null);
    setTooLarge(false);
    void (async () => {
      try {
        const res = await workingFileApi.readBytes(path);
        if (disposed) return;
        setSize(res.size);
        if (res.too_large) {
          setTooLarge(true);
          return;
        }
        const blob = base64ToBlob(res.base64, res.mime);
        const objUrl = URL.createObjectURL(blob);
        urlRef.current = objUrl;
        setUrl(objUrl);
      } catch (e) {
        if (!disposed) setError(e instanceof Error ? e.message : String(e));
      }
    })();
    return () => {
      disposed = true;
      if (urlRef.current) {
        URL.revokeObjectURL(urlRef.current);
        urlRef.current = null;
      }
    };
  }, [path]);

  if (error)
    return (
      <p className="cw-error" role="alert">
        {error}
      </p>
    );
  if (tooLarge)
    return (
      <p className="cw-empty">
        Model is {formatSize(size)} — too large to preview inline.
      </p>
    );
  if (!url) return <p className="cw-status">Loading model…</p>;
  if (!MODEL_EXTS.has(ext))
    return <BinaryFallback path={path} />;

  return (
    <Suspense fallback={<p className="cw-status">Loading 3D viewer…</p>}>
      <ModelViewer url={url} ext={ext} />
    </Suspense>
  );
}

/** Unknown/binary: metadata-only card. */
function BinaryFallback({ path }: { path: string }) {
  const [meta, setMeta] = useState<{ size: number; mime: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    void (async () => {
      try {
        const res = await workingFileApi.readBytes(path);
        if (!disposed) setMeta({ size: res.size, mime: res.mime });
      } catch (e) {
        if (!disposed) setError(e instanceof Error ? e.message : String(e));
      }
    })();
    return () => {
      disposed = true;
    };
  }, [path]);

  if (error)
    return (
      <p className="cw-error" role="alert">
        {error}
      </p>
    );
  return (
    <div className="cw-fallback">
      <p className="cw-empty">No inline preview for this file type.</p>
      <dl className="cw-meta-dl">
        <dt>Name</dt>
        <dd>{path.split(/[\\/]/).pop()}</dd>
        <dt>Type</dt>
        <dd>{meta?.mime ?? "—"}</dd>
        <dt>Size</dt>
        <dd>{meta ? formatSize(meta.size) : "—"}</dd>
      </dl>
    </div>
  );
}
