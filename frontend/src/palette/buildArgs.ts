/**
 * Pure (React-free) form-value helpers for the command palette.
 *
 * Extracted from `form.tsx` so the arg-marshalling logic can be unit-tested with
 * Node's built-in test runner (no React/DOM import). `form.tsx` re-uses these.
 */
import type { FieldSpec, OpManifest } from "./types";

/** Editable representation of a field value while the form is open. */
export type EditValue = string | boolean;

/** Build the initial edit-time values record for a manifest's fields. */
export function initialValues(manifest: OpManifest): Record<string, EditValue> {
  const out: Record<string, EditValue> = {};
  for (const f of manifest.args) {
    if (f.kind === "boolean") {
      out[f.name] = typeof f.default === "boolean" ? f.default : false;
    } else if (f.kind === "string-list") {
      out[f.name] = Array.isArray(f.default) ? f.default.join("\n") : "";
    } else if (f.default !== undefined) {
      out[f.name] = String(f.default);
    } else {
      out[f.name] = "";
    }
  }
  return out;
}

/** Whether a field currently holds a non-empty value. */
export function fieldFilled(f: FieldSpec, v: EditValue): boolean {
  if (f.kind === "boolean") return true;
  if (f.kind === "string-list") {
    return String(v)
      .split("\n")
      .some((line) => line.trim().length > 0);
  }
  return String(v).trim().length > 0;
}

/**
 * Convert the edit-time values into the typed args object for `invoke`.
 *
 * Every field ALWAYS emits a key, even when blank-and-optional. The Rust
 * `#[tauri::command]` params are plain (non-`Option`) types, so Tauri rejects
 * the whole invoke with "missing required key" if an expected key is absent —
 * which used to drop ~25 commands' optional args. Lore ops treat an empty
 * string as "use the repo default", so a blank optional text/enum field is sent
 * as `""` rather than omitted. Numbers always send a concrete value too.
 */
export function buildArgs(
  manifest: OpManifest,
  values: Record<string, EditValue>,
): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const f of manifest.args) {
    const v = values[f.name];
    const filled = fieldFilled(f, v);
    switch (f.kind) {
      case "boolean":
        out[f.name] = Boolean(v);
        break;
      case "number":
        // Always emit the key. Filled → the parsed number; blank → the field's
        // default (when given), else 0, so Tauri never sees a missing key.
        if (filled) out[f.name] = Number(v);
        else if (typeof f.default === "number") out[f.name] = f.default;
        else out[f.name] = 0;
        break;
      case "string-list":
        out[f.name] = String(v)
          .split("\n")
          .map((s) => s.trim())
          .filter((s) => s.length > 0);
        break;
      default: // text | enum
        // Always emit the key — empty string when blank+optional.
        out[f.name] = String(v);
        break;
    }
  }
  return out;
}
