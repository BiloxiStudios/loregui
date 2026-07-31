import { open } from "@tauri-apps/plugin-dialog";
import { documentDir, homeDir } from "@tauri-apps/api/path";
import { isAcceptableAbsolutePath } from "./pathPolicy";

export interface DirectoryPickerOptions {
  title: string;
  defaultPath?: string;
}

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
 * throw) — i.e. we genuinely have nothing to offer, not merely because the
 * field was empty or held a relative value.
 */
export async function defaultDialogPath(): Promise<string | undefined> {
  for (const resolve of [documentDir, homeDir]) {
    try {
      const candidate = await resolve();
      if (candidate && isAcceptableAbsolutePath(candidate)) return candidate;
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
 * The dialog starts at `options.defaultPath` only when that value is an
 * acceptable absolute path (see `pathPolicy`); an empty, whitespace, or
 * relative value would otherwise be handed to the OS, which resolves it
 * against the process CWD. In that case we start from
 * {@link defaultDialogPath} instead.
 */
export async function chooseDirectory(
  options: DirectoryPickerOptions,
): Promise<string | null> {
  const { defaultPath, ...rest } = options;
  const start =
    defaultPath !== undefined && isAcceptableAbsolutePath(defaultPath)
      ? defaultPath.trim()
      : await defaultDialogPath();
  const selected = await open({
    directory: true,
    multiple: false,
    ...rest,
    defaultPath: start,
  });
  return typeof selected === "string" ? selected : null;
}
