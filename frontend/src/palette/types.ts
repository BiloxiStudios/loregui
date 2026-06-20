/**
 * Command-palette manifest model.
 *
 * Every lore-vm op the GUI can invoke is described by one {@link OpManifest}
 * (one file per op under `manifest/<domain>/<op>.ts`). The palette renders a
 * form from the op's {@link FieldSpec}s, collects values, and invokes the named
 * Tauri command. This is the single unit of parity work: add a manifest entry
 * and the op becomes reachable from Ctrl-K with a generated form.
 */

export type FieldKind =
  | "text"
  | "number"
  | "boolean"
  | "enum"
  | "string-list";

/** One argument of an op, rendered as one form control. */
export interface FieldSpec {
  /**
   * The argument key sent to the Tauri command. Use the exact camelCase key the
   * command expects (Tauri v2 maps camelCase JS keys to snake_case Rust args),
   * matching the corresponding `api.ts` call site.
   */
  name: string;
  kind: FieldKind;
  /** Control label. */
  label: string;
  /** Optional helper text shown under the control. */
  description?: string;
  /** Required fields block submit until filled. Defaults to false. */
  required?: boolean;
  /** Initial value. For `string-list` an array; for `boolean` a bool. */
  default?: string | number | boolean | string[];
  placeholder?: string;
  /** Options for `kind === "enum"`. */
  options?: { value: string; label: string }[];
}

/** How the palette renders a command's return value. */
export type ResultKind =
  | "void" // no meaningful return — show "Done"
  | "text" // a plain string
  | "json"; // anything else — pretty-printed (default)

/** A single invokable op. */
export interface OpManifest {
  /** Stable id `"<domain>.<op>"` — used for search, sort, and dedupe. */
  id: string;
  /** Op domain, e.g. "repository". */
  domain: string;
  /** Op leaf name, e.g. "status". */
  op: string;
  /** Human label shown in the palette, e.g. "Repository: Status". */
  label: string;
  /** One-line description of what the op does. */
  description?: string;
  /** The registered Tauri command name to invoke. */
  command: string;
  /** Ordered argument fields → generated form. Empty for no-arg ops. */
  args: FieldSpec[];
  /** Result rendering hint. Defaults to "json". */
  resultKind?: ResultKind;
  /** Extra search terms beyond id/label/description. */
  keywords?: string[];
}

/** Build a default form-values record from a manifest's field specs. */
export function defaultValues(manifest: OpManifest): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const f of manifest.args) {
    if (f.default !== undefined) {
      out[f.name] = f.default;
    } else if (f.kind === "boolean") {
      out[f.name] = false;
    } else if (f.kind === "string-list") {
      out[f.name] = [];
    } else if (f.kind === "number") {
      out[f.name] = "";
    } else {
      out[f.name] = "";
    }
  }
  return out;
}
