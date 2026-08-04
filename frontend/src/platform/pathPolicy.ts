/**
 * SBAI-5841 — lexical absolute-path policy for lifecycle paths, in the UI.
 *
 * This is the **supplemental** half of the fail-closed absolute-path policy.
 * It mirrors `src-tauri/src/path_policy.rs` so the user sees an actionable
 * message *before* the round-trip, but it is deliberately **not** a trust
 * boundary: the backend re-validates every store directory and every
 * repository open/create/clone destination and is the only authority. Never
 * treat a pass here as permission to skip the backend check.
 *
 * Why it exists at all: a packaged app can start with an arbitrary CWD (on
 * Windows, launching from the Start Menu can leave it at
 * `C:\Windows\System32`), so a relative value like `.` or `lore` would
 * silently resolve somewhere the user never intended. Telling the user that
 * inline — at the field — is far better UX than a backend rejection string.
 *
 * **Union semantics (the one deliberate divergence from the backend).** The
 * backend classifies for its own compile target; the frontend cannot reliably
 * know the target OS, so a value is acceptable here when it is acceptable on
 * *either* family:
 *
 * | input                          | verdict | why                              |
 * |--------------------------------|---------|----------------------------------|
 * | `/srv/lore`, `/with spaces/x`  | accept  | unix-rooted                      |
 * | `C:\LoreData`, `C:/x`, `C:\`   | accept  | windows drive-absolute           |
 * | `\\server\share[\…]`           | accept  | UNC with both server and share   |
 * | `\\?\C:\x`, `\\?\UNC\srv\shr`  | accept  | verbatim disk / verbatim UNC     |
 * | `""`, `"   "`, `.`, `..`, `x`  | reject  | resolves against the startup dir |
 * | `C:foo`, `C:`                  | reject  | drive-relative                   |
 * | `\foo`                         | reject  | driveless rootful (Windows form) |
 * | `\\server`, `\\`               | reject  | incomplete UNC (no share)        |
 * | `\\.\COM1`                     | reject  | device namespace, not a folder   |
 * | `\\?\lore`                     | reject  | unsupported verbatim form        |
 *
 * Note the intended edge: `/foo` is *rejected* by the backend's Windows
 * classifier (driveless rootful) but *accepted* here, because it is a
 * perfectly good POSIX absolute path and the union accepts it. On Windows the
 * backend still gets the last word.
 *
 * **Two checks, two jobs — do not mix them up.**
 *
 * - {@link explainPathProblem} / {@link isAcceptableAbsolutePath} — the union.
 *   *Preflight UX only, non-authorizing.* Their leniency exists so the inline
 *   field error never scolds a user for a path the backend would accept.
 * - {@link isNativeAbsolutePath} — the current platform only. *Authorizing.*
 *   Anything handed to the OS (a folder picker's starting directory) must
 *   clear this one, because a union pass is not evidence the value means
 *   anything on the machine we are actually running on.
 */

/** Why a candidate failed. Mirrors `PathRejection` in path_policy.rs. */
export type PathRejection =
  | "empty"
  | "relative"
  | "driveRelative"
  | "rootRelative"
  | "deviceNamespace"
  | "incompleteUnc"
  | "unsupportedVerbatim";

const isSep = (ch: string): boolean => ch === "\\" || ch === "/";

const isDriveLetter = (ch: string): boolean =>
  (ch >= "a" && ch <= "z") || (ch >= "A" && ch <= "Z");

/** `\\?\`-style verbatim prefix (either separator accepted lexically). */
function stripVerbatimPrefix(value: string): string | null {
  if (
    value.length >= 4 &&
    isSep(value[0]) &&
    isSep(value[1]) &&
    value[2] === "?" &&
    isSep(value[3])
  ) {
    return value.slice(4);
  }
  return null;
}

/**
 * `X:\…` or `X:/…` (drive letter, colon, separator). Bare `X:` is
 * drive-relative and must NOT match.
 */
function isDriveAbsolute(value: string): boolean {
  return (
    value.length >= 3 &&
    isDriveLetter(value[0]) &&
    value[1] === ":" &&
    isSep(value[2])
  );
}

/**
 * After a UNC prefix: require a non-empty server, a separator, and a non-empty
 * share component.
 */
function requireServerAndShare(rest: string): PathRejection | null {
  const parts = rest.split(/[\\/]/);
  const server = parts[0] ?? "";
  const share = parts[1] ?? "";
  return server.length === 0 || share.length === 0 ? "incompleteUnc" : null;
}

/**
 * Lexically classify `candidate` for one target family. `null` = acceptable.
 * Mirrors `path_policy::classify`; callers pass an already-trimmed value.
 */
export function classify(
  candidate: string,
  windows: boolean,
): PathRejection | null {
  if (candidate.length === 0) return "empty";

  if (!windows) {
    return candidate.startsWith("/") ? null : "relative";
  }

  // Verbatim namespace: `\\?\C:\…` or `\\?\UNC\server\share…`.
  const verbatim = stripVerbatimPrefix(candidate);
  if (verbatim !== null) {
    if (isDriveAbsolute(verbatim)) return null;
    if (verbatim.startsWith("UNC\\") || verbatim.startsWith("UNC/")) {
      return requireServerAndShare(verbatim.slice(4));
    }
    return "unsupportedVerbatim";
  }

  // Device namespace: `\\.\COM1` and friends. Never a store directory.
  if (
    candidate.length >= 4 &&
    isSep(candidate[0]) &&
    isSep(candidate[1]) &&
    candidate[2] === "." &&
    isSep(candidate[3])
  ) {
    return "deviceNamespace";
  }

  // UNC: `\\server\share[\…]` — both components required.
  if (candidate.length >= 2 && isSep(candidate[0]) && isSep(candidate[1])) {
    return requireServerAndShare(candidate.slice(2));
  }

  // Drive-letter forms.
  if (candidate.length >= 2 && isDriveLetter(candidate[0]) && candidate[1] === ":") {
    return isDriveAbsolute(candidate) ? null : "driveRelative";
  }

  // Rootful but driveless: `\foo` — relative to the current drive.
  if (isSep(candidate[0])) return "rootRelative";

  return "relative";
}

/**
 * Human-actionable explanation for a rejection — says what the value is and
 * what to type instead. Mirrors `path_policy::rejection_message` minus the
 * backend's field-role prefix (the field label already supplies that here).
 */
export function rejectionMessage(
  value: string,
  rejection: PathRejection,
): string {
  switch (rejection) {
    case "empty":
      return (
        "A folder is required — enter a full absolute path such as " +
        "C:\\LoreData or /home/you/lore"
      );
    case "relative":
      return (
        `"${value}" is a relative path and would resolve against the app's ` +
        "startup folder (which can be C:\\Windows\\System32) — enter a full " +
        "absolute path such as C:\\LoreData or /home/you/lore"
      );
    case "driveRelative":
      return (
        `"${value}" is drive-relative (it depends on that drive's current ` +
        "folder) — add a backslash after the drive letter, for example " +
        "C:\\LoreData"
      );
    case "rootRelative":
      return (
        `"${value}" does not name a drive — include the drive letter, for ` +
        "example C:\\LoreData"
      );
    case "deviceNamespace":
      return (
        `"${value}" is a Windows device path, not a folder — choose a normal ` +
        "folder such as C:\\LoreData"
      );
    case "incompleteUnc":
      return (
        `"${value}" is an incomplete network path — include both server and ` +
        "share, for example \\\\server\\share\\lore"
      );
    case "unsupportedVerbatim":
      return (
        `"${value}" uses an unsupported \\\\?\\ form — use a drive path such ` +
        "as \\\\?\\C:\\LoreData, or drop the \\\\?\\ prefix"
      );
  }
}

/**
 * Explain why `value` is not an acceptable absolute lifecycle path, or `null`
 * when it is acceptable. Operates on `value.trim()`, so whitespace-only input
 * reads as empty — exactly like `path_policy::require_absolute`.
 *
 * Acceptance is the **union** of the unix and Windows classifiers (see the
 * module doc); the reported reason is the Windows one, which is always the
 * more specific of the two for a value both families reject.
 *
 * **Non-authorizing.** This is preflight *UX* only — it exists to tell the
 * user what to fix before they submit, and it is deliberately lenient because
 * the UI cannot know the target OS. Never use it to decide what to hand to the
 * OS or to the backend; use {@link isNativeAbsolutePath} for that.
 */
export function explainPathProblem(value: string): string | null {
  const trimmed = value.trim();
  if (classify(trimmed, false) === null) return null;
  const windowsRejection = classify(trimmed, true);
  if (windowsRejection === null) return null;
  return rejectionMessage(trimmed, windowsRejection);
}

/**
 * Non-erroring form of {@link explainPathProblem}: would this value survive
 * the policy on *some* platform?
 *
 * **Non-authorizing** — same caveat as `explainPathProblem`. It answers "is
 * this worth complaining about in the field?", not "is this safe to act on".
 * Anything that reaches the OS (a picker's starting folder) or stands in for a
 * real location must go through {@link isNativeAbsolutePath}.
 */
export function isAcceptableAbsolutePath(value: string): boolean {
  return explainPathProblem(value) === null;
}

/**
 * Is the process running on Windows? Detected from the user agent, which is
 * the only signal available without pulling in a Tauri OS plugin. Guarded so a
 * non-DOM context (SSR, a bare Node test runner) simply reads as "not
 * Windows" — the stricter of the two answers for a POSIX-shaped check.
 */
export function isWindowsPlatform(): boolean {
  if (typeof navigator === "undefined") return false;
  const ua = navigator.userAgent;
  return typeof ua === "string" && /\bWindows\b/.test(ua);
}

/**
 * Is `value` absolute **on the platform actually running this app**?
 *
 * This is the **authorizing** check — the one to use for anything handed to
 * the OS, above all a folder picker's starting directory. The union helpers
 * ({@link explainPathProblem} / {@link isAcceptableAbsolutePath}) deliberately
 * accept the other family's forms so the inline field error stays quiet for a
 * value the backend might well accept; that leniency is fine for a message and
 * wrong for an action. Concretely, on Windows `/foo` and `\foo` are
 * driveless-rootful and resolve against the *current drive* — union-acceptable,
 * never a trusted starting folder.
 *
 * `windows` defaults to {@link isWindowsPlatform} and is injectable so both
 * platform tables stay testable on one host.
 */
export function isNativeAbsolutePath(
  value: string,
  windows: boolean = isWindowsPlatform(),
): boolean {
  return classify(value.trim(), windows) === null;
}
