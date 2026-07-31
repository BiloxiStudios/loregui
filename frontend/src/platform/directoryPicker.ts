import { open } from "@tauri-apps/plugin-dialog";
import { documentDir, homeDir } from "@tauri-apps/api/path";
import { isNativeAbsolutePath } from "./pathPolicy";

export interface DirectoryPickerOptions {
  title: string;
  defaultPath?: string;
}

/**
 * Thrown (and shown) when there is no trusted folder to start the dialog from.
 * Opening anyway would let the OS fall back to the process working directory,
 * so the picker refuses instead — fail closed, and tell the user the way out.
 */
export const NO_TRUSTED_START_MESSAGE =
  "Cannot open the folder picker: no trusted starting folder is available " +
  "(Documents and home lookup failed) — enter an absolute path manually.";

/**
 * SBAI-5841: where a folder picker starts when we have nothing better.
 *
 * Opening the native dialog with no `defaultPath` lets the OS fall back to the
 * process working directory, which for a packaged build can be anything (on
 * Windows, `C:\Windows\System32` when launched from the Start Menu). That is
 * how a user ends up "browsing" straight into a system folder and picking a
 * store location nobody intended. So we always aim at a deliberate,
 * user-writable place instead: Documents, then the home directory.
 *
 * Returns `undefined` only when the path API itself is unavailable (both calls
 * throw or answer with something we cannot trust) — i.e. we genuinely have
 * nothing to offer, not merely because the field was empty or held a relative
 * value. Callers must treat `undefined` as "do not open the dialog".
 *
 * Candidates are vetted with the **native** check, not the union: a starting
 * folder is an action, and a path that is absolute only on the *other* OS
 * family is no more usable here than a relative one.
 */
export async function defaultDialogPath(): Promise<string | undefined> {
  for (const resolve of [documentDir, homeDir]) {
    try {
      const candidate = await resolve();
      if (candidate && isNativeAbsolutePath(candidate)) return candidate;
    } catch {
      // Fall through to the next candidate.
    }
  }
  return undefined;
}

/**
 * Open the native directory picker and return the chosen absolute path, or
 * `null` when the user cancels (cancel leaves caller state untouched).
 *
 * The dialog starts at `options.defaultPath` only when that value is absolute
 * *on this platform* (see `isNativeAbsolutePath`); an empty, whitespace,
 * relative, or foreign-family value would otherwise be handed to the OS, which
 * resolves it against the process CWD. In that case we start from
 * {@link defaultDialogPath} instead.
 *
 * If neither yields a trusted folder the picker **fails closed**: it throws
 * {@link NO_TRUSTED_START_MESSAGE} without ever calling the dialog, because
 * opening with no `defaultPath` is exactly the CWD inheritance this guards
 * against. Callers surface the message and let the user type a path instead.
 */
export async function chooseDirectory(
  options: DirectoryPickerOptions,
): Promise<string | null> {
  const { defaultPath, ...rest } = options;
  const start =
    defaultPath !== undefined && isNativeAbsolutePath(defaultPath)
      ? defaultPath.trim()
      : await defaultDialogPath();
  if (start === undefined) {
    throw new Error(NO_TRUSTED_START_MESSAGE);
  }
  const selected = await open({
    directory: true,
    multiple: false,
    ...rest,
    defaultPath: start,
  });
  return typeof selected === "string" ? selected : null;
}
