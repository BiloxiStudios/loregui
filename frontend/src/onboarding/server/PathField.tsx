import { useCallback, useState } from "react";
import type { ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  defaultDialogPath,
  NO_TRUSTED_START_MESSAGE,
} from "../../platform/directoryPicker";
import {
  explainPathProblem,
  isNativeAbsolutePath,
} from "../../platform/pathPolicy";

interface PathFieldBaseProps {
  /** Input id — the visible label points at it via htmlFor. */
  id: string;
  /** Field label. In read-only mode this is the role label for the summary. */
  label: string;
  /** Current path value. */
  value: string;
  /** Hint copy shown under the field. */
  hint?: ReactNode;
  /** Marks the field optional (styles only). */
  optional?: boolean;
}

interface PathFieldEditableProps extends PathFieldBaseProps {
  readOnly?: false;
  /** Called with the new value on manual edits and on picker selection. */
  onChange: (value: string) => void;
  placeholder?: string;
  /** Title of the native directory dialog. */
  dialogTitle: string;
  disabled?: boolean;
}

interface PathFieldReadOnlyProps extends PathFieldBaseProps {
  /** Read-only summary: renders the path as static text with its role label. */
  readOnly: true;
}

export type PathFieldProps = PathFieldEditableProps | PathFieldReadOnlyProps;

/**
 * Shared directory-path field for the host-a-server flow (SBAI-5560). Two
 * variants:
 *
 * - **Editable** (default): label + text input + "Browse…" button that opens
 *   the native directory picker (`@tauri-apps/plugin-dialog`, directory mode)
 *   and fills the input with the selection. Used in step 1, the ONLY place a
 *   store path is asked.
 * - **Read-only summary** (`readOnly`): renders the path as static text under
 *   its role label — used downstream so later steps display the path chosen in
 *   step 1 without re-asking.
 */
export default function PathField(props: PathFieldProps) {
  const { id, label, value, hint, optional } = props;
  // Editable-only props, present unless this is a read-only summary.
  const onChange = props.readOnly ? undefined : props.onChange;
  const dialogTitle = props.readOnly ? undefined : props.dialogTitle;
  const [browsing, setBrowsing] = useState(false);
  const [browseError, setBrowseError] = useState<string | null>(null);

  const handleBrowse = useCallback(async () => {
    if (!onChange) return;
    setBrowsing(true);
    try {
      // SBAI-5841: never hand the OS an empty, relative, or foreign-platform
      // starting point — it resolves that against the process CWD (System32
      // for a Start Menu launch). Reuse the current value only when it is
      // absolute *on this platform*, and resolve the whole starting folder
      // BEFORE opening, so "no trusted start" never becomes "open at the CWD".
      const defaultPath = isNativeAbsolutePath(value)
        ? value.trim()
        : await defaultDialogPath();
      if (defaultPath === undefined) {
        setBrowseError(NO_TRUSTED_START_MESSAGE);
        return;
      }
      const selected = await open({
        directory: true,
        multiple: false,
        title: dialogTitle,
        defaultPath,
      });
      if (typeof selected === "string") {
        setBrowseError(null);
        onChange(selected);
      }
    } finally {
      setBrowsing(false);
    }
  }, [onChange, dialogTitle, value]);

  // SBAI-5841: supplemental inline validation. The backend is the authority
  // and re-checks every lifecycle path; this just tells the user what to fix
  // before they submit. Stays quiet on an untouched (empty) field — "required"
  // is the submit button's job, not a red error on first paint.
  const pathProblem =
    props.readOnly || value.trim().length === 0
      ? null
      : explainPathProblem(value);
  const errorId = `${id}-path-error`;
  const browseErrorId = `${id}-browse-error`;
  const describedBy =
    [pathProblem ? errorId : null, browseError ? browseErrorId : null]
      .filter((part): part is string => part !== null)
      .join(" ") || undefined;

  const fieldClass = `onboarding-field${optional ? " onboarding-field--optional" : ""}${
    pathProblem || browseError ? " server-config-field--invalid" : ""
  }`;

  if (props.readOnly) {
    return (
      <div className={fieldClass}>
        <span>{label}</span>
        <code>{value || "Not set"}</code>
        {hint ? <span className="onboarding-field-hint">{hint}</span> : null}
      </div>
    );
  }

  const disabled = props.disabled || browsing;
  return (
    <div className={fieldClass}>
      <label htmlFor={id}>{label}</label>
      <div className="onboarding-url-row">
        <input
          id={id}
          type="text"
          placeholder={props.placeholder}
          value={value}
          onChange={(e) => {
            // A manual edit is the user answering the browse failure — drop it.
            setBrowseError(null);
            props.onChange(e.target.value);
          }}
          disabled={disabled}
          aria-invalid={pathProblem ? true : undefined}
          aria-describedby={describedBy}
        />
        <button
          type="button"
          className="onboarding-button"
          onClick={() => void handleBrowse()}
          disabled={disabled}
        >
          {browsing ? "Browsing…" : "Browse…"}
        </button>
      </div>
      {pathProblem ? (
        <p className="server-config-field-error" id={errorId}>
          {pathProblem}
        </p>
      ) : null}
      {browseError ? (
        <p className="server-config-field-error" id={browseErrorId}>
          {browseError}
        </p>
      ) : null}
      {hint ? <span className="onboarding-field-hint">{hint}</span> : null}
    </div>
  );
}
