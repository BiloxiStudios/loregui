import { useMemo, useState } from "react";
import type { CSSProperties } from "react";
import type { FieldSpec, OpManifest } from "./types";

/** Editable representation of a field value while the form is open. */
type EditValue = string | boolean;

function initialValues(manifest: OpManifest): Record<string, EditValue> {
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

function fieldFilled(f: FieldSpec, v: EditValue): boolean {
  if (f.kind === "boolean") return true;
  if (f.kind === "string-list") {
    return String(v)
      .split("\n")
      .some((line) => line.trim().length > 0);
  }
  return String(v).trim().length > 0;
}

/** Convert the edit-time values into the typed args object for `invoke`. */
function buildArgs(
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
        if (filled) out[f.name] = Number(v);
        else if (f.required) out[f.name] = Number(v);
        break;
      case "string-list":
        out[f.name] = String(v)
          .split("\n")
          .map((s) => s.trim())
          .filter((s) => s.length > 0);
        break;
      default: // text | enum
        if (filled || f.required) out[f.name] = String(v);
        break;
    }
  }
  return out;
}

const labelStyle: CSSProperties = {
  display: "block",
  fontSize: 12,
  fontWeight: 600,
  marginBottom: 4,
  color: "var(--surface-base-text)",
};
const descStyle: CSSProperties = {
  fontSize: 11,
  color: "var(--surface-base-text-secondary)",
  marginTop: 2,
};
const inputStyle: CSSProperties = {
  width: "100%",
  boxSizing: "border-box",
  padding: "6px 8px",
  background: "var(--surface-input-bg)",
  color: "var(--surface-input-text)",
  border: "1px solid var(--surface-input-border)",
  borderRadius: 4,
  fontSize: 13,
};

interface OpFormProps {
  manifest: OpManifest;
  running: boolean;
  onRun: (args: Record<string, unknown>) => void;
}

/** Renders a generated form from an op's {@link FieldSpec}s and runs it. */
export default function OpForm({ manifest, running, onRun }: OpFormProps) {
  const [values, setValues] = useState<Record<string, EditValue>>(() =>
    initialValues(manifest),
  );

  const valid = useMemo(
    () =>
      manifest.args
        .filter((f) => f.required)
        .every((f) => fieldFilled(f, values[f.name])),
    [manifest, values],
  );

  const set = (name: string, v: EditValue) =>
    setValues((prev) => ({ ...prev, [name]: v }));

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (valid && !running) onRun(buildArgs(manifest, values));
      }}
    >
      {manifest.args.length === 0 && (
        <p style={{ ...descStyle, marginBottom: 12 }}>
          This command takes no arguments.
        </p>
      )}

      {manifest.args.map((f) => (
        <div key={f.name} style={{ marginBottom: 12 }}>
          {f.kind === "boolean" ? (
            <label
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                cursor: "pointer",
              }}
            >
              <input
                type="checkbox"
                checked={Boolean(values[f.name])}
                onChange={(e) => set(f.name, e.target.checked)}
                disabled={running}
              />
              <span style={{ ...labelStyle, marginBottom: 0 }}>
                {f.label}
                {f.required ? " *" : ""}
              </span>
            </label>
          ) : (
            <>
              <label style={labelStyle} htmlFor={`pf-${f.name}`}>
                {f.label}
                {f.required ? " *" : ""}
              </label>
              {f.kind === "enum" ? (
                <select
                  id={`pf-${f.name}`}
                  style={inputStyle}
                  value={String(values[f.name])}
                  onChange={(e) => set(f.name, e.target.value)}
                  disabled={running}
                >
                  <option value="">—</option>
                  {(f.options ?? []).map((o) => (
                    <option key={o.value} value={o.value}>
                      {o.label}
                    </option>
                  ))}
                </select>
              ) : f.kind === "string-list" ? (
                <textarea
                  id={`pf-${f.name}`}
                  style={{ ...inputStyle, minHeight: 64, fontFamily: "inherit" }}
                  placeholder={f.placeholder ?? "one value per line"}
                  value={String(values[f.name])}
                  onChange={(e) => set(f.name, e.target.value)}
                  disabled={running}
                />
              ) : (
                <input
                  id={`pf-${f.name}`}
                  type={f.kind === "number" ? "number" : "text"}
                  style={inputStyle}
                  placeholder={f.placeholder}
                  value={String(values[f.name])}
                  onChange={(e) => set(f.name, e.target.value)}
                  disabled={running}
                />
              )}
            </>
          )}
          {f.description && <div style={descStyle}>{f.description}</div>}
        </div>
      ))}

      <button
        type="submit"
        disabled={!valid || running}
        style={{
          padding: "7px 14px",
          background: "var(--surface-primary-bg)",
          color: "var(--surface-primary-text)",
          border: "1px solid var(--surface-primary-border)",
          borderRadius: 4,
          fontWeight: 600,
          cursor: valid && !running ? "pointer" : "not-allowed",
          opacity: valid && !running ? 1 : 0.5,
        }}
      >
        {running ? "Running…" : "Run"}
      </button>
    </form>
  );
}
