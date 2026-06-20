import type { CSSProperties } from "react";
import type { ResultKind } from "./types";

const boxStyle: CSSProperties = {
  marginTop: 12,
  padding: "10px 12px",
  background: "var(--surface-base-bg)",
  border: "1px solid var(--surface-base-border)",
  borderRadius: 4,
  fontSize: 12,
};

const preStyle: CSSProperties = {
  ...boxStyle,
  margin: "12px 0 0",
  maxHeight: 280,
  overflow: "auto",
  whiteSpace: "pre-wrap",
  wordBreak: "break-word",
  fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
  color: "var(--surface-base-text)",
};

interface OpResultProps {
  value: unknown;
  kind?: ResultKind;
}

/** Renders a command's return value per its {@link ResultKind} hint. */
export default function OpResult({ value, kind = "json" }: OpResultProps) {
  if (kind === "void") {
    return (
      <div style={{ ...boxStyle, color: "var(--surface-success-text)" }}>
        ✓ Done
      </div>
    );
  }

  if (kind === "text") {
    const text =
      value === null || value === undefined ? "" : String(value);
    return (
      <div style={{ ...boxStyle, color: "var(--surface-base-text)" }}>
        {text || <em style={{ color: "var(--surface-base-text-secondary)" }}>empty</em>}
      </div>
    );
  }

  // json (default)
  let pretty: string;
  try {
    pretty = JSON.stringify(value, null, 2);
  } catch {
    pretty = String(value);
  }
  if (pretty === undefined) pretty = "undefined";
  return <pre style={preStyle}>{pretty}</pre>;
}
