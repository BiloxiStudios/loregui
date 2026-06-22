// File-kind detection for the content workspace (SBAI-4083/4084/4085).
//
// Maps a path's extension to the renderer the Preview tab should use, and to a
// CodeMirror language for Edit / text preview. Pure, framework-free, and the
// single source of truth so Preview / Diff / Edit agree on how to treat a file.

export type ContentKind =
  | "image"
  | "model"
  | "audio"
  | "video"
  | "text"
  | "binary";

const IMAGE = new Set(["png", "jpg", "jpeg", "webp", "gif", "svg", "bmp", "ico"]);
const MODEL = new Set(["gltf", "glb", "fbx", "obj"]);
const AUDIO = new Set(["wav", "mp3", "ogg", "flac", "m4a"]);
const VIDEO = new Set(["mp4", "webm", "mov"]);

// Extensions we confidently treat as text/code even with no language mode.
const TEXT = new Set([
  "txt", "md", "markdown", "rs", "toml", "json", "jsonc", "js", "jsx", "ts",
  "tsx", "mjs", "cjs", "css", "scss", "html", "htm", "xml", "yaml", "yml",
  "sh", "bash", "zsh", "py", "go", "c", "h", "cpp", "hpp", "cc", "java",
  "kt", "swift", "rb", "php", "sql", "ini", "cfg", "conf", "env", "log",
  "gitignore", "lock", "csv", "tsv", "lore", "uasset.txt",
]);

export function extOf(path: string): string {
  const base = path.split(/[\\/]/).pop() ?? path;
  const dot = base.lastIndexOf(".");
  if (dot <= 0) return "";
  return base.slice(dot + 1).toLowerCase();
}

export function kindOf(path: string): ContentKind {
  const ext = extOf(path);
  if (IMAGE.has(ext)) return "image";
  if (MODEL.has(ext)) return "model";
  if (AUDIO.has(ext)) return "audio";
  if (VIDEO.has(ext)) return "video";
  if (TEXT.has(ext)) return "text";
  return "binary";
}

/** CodeMirror language id for an extension, or null for plain text. */
export type CmLang = "javascript" | "typescript" | "json" | "markdown" | "rust" | null;

export function cmLangOf(path: string): CmLang {
  const ext = extOf(path);
  switch (ext) {
    case "js":
    case "jsx":
    case "mjs":
    case "cjs":
      return "javascript";
    case "ts":
    case "tsx":
      return "typescript";
    case "json":
    case "jsonc":
      return "json";
    case "md":
    case "markdown":
      return "markdown";
    case "rs":
      return "rust";
    default:
      return null;
  }
}

/** Human-readable file size. */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
}

/** Decode a base64 string to a Blob with the given MIME (for object URLs). */
export function base64ToBlob(b64: string, mime: string): Blob {
  const bin = atob(b64);
  const len = bin.length;
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i += 1) bytes[i] = bin.charCodeAt(i);
  return new Blob([bytes], { type: mime || "application/octet-stream" });
}
