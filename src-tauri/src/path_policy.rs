//! SBAI-5841: fail-closed absolute-path policy for lifecycle paths.
//!
//! Every user-supplied or persisted *lifecycle* path — host/mutable store
//! roots, repository open/create/clone targets, and the persisted active
//! repository context — must be validated here **before** any filesystem
//! write, config emission, child spawn, or state mutation. A packaged
//! process can start with an arbitrary CWD (on Windows, launching from the
//! Start Menu or a shell can leave it at `C:\Windows\System32`), so a
//! relative value like `.` or `lore` would silently resolve into System32.
//!
//! The check is purely **lexical**: create targets do not exist yet, so no
//! `canonicalize`/filesystem access is allowed here. Working-tree file paths
//! are the opposite contract (always repo-relative) and are handled by
//! `resolve_in_working_tree` in `commands.rs`, not by this module.
//!
//! Windows semantics (deliberate, tested on all platforms via the
//! target-parameterized classifier):
//!
//! | input                     | verdict | why                                    |
//! |---------------------------|---------|----------------------------------------|
//! | `C:\LoreData`, `C:/x`     | accept  | drive-absolute                         |
//! | `C:\` (drive root)        | accept  | absolute; unusual but explicit         |
//! | `\\server\share[\...]`    | accept  | UNC with both server and share         |
//! | `\\?\C:\x`, `\\?\UNC\s\s` | accept  | verbatim disk / verbatim UNC           |
//! | `C:foo`                   | reject  | drive-relative: depends on per-drive CWD |
//! | `\foo`, `/foo`            | reject  | rootful but drive-relative on Windows  |
//! | `\\server`, `\\s\` `\\`   | reject  | incomplete UNC (no share)              |
//! | `\\.\COM1` (device NS)    | reject  | not a usable folder                    |
//! | `.`, `..`, `lore`, empty  | reject  | resolves against process CWD           |
//!
//! Unix accepts only rooted paths (`/...`). Interior `.`/`..` components in
//! an otherwise-absolute path are permitted: they resolve deterministically
//! without consulting the process CWD, which is the trust boundary here.

use std::path::PathBuf;

use lore_vm::LoreError;

/// Why a candidate path failed the policy. Carried so every caller surfaces
/// the same actionable message for the same defect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathRejection {
    /// Empty input ([`require_absolute`] trims first, so whitespace-only
    /// input reaches [`classify`] as empty).
    Empty,
    /// Plain relative path: `.`, `..`, `lore`, `./x`, …
    Relative,
    /// Windows drive-relative: `C:foo` — resolves against that drive's CWD.
    DriveRelative,
    /// Windows rootful-but-driveless: `\foo` or `/foo` — resolves against the
    /// current drive.
    RootRelative,
    /// Windows device namespace (`\\.\…`) — not a usable directory.
    DeviceNamespace,
    /// UNC prefix without both a server and a share (`\\server`, `\\`).
    IncompleteUnc,
    /// Verbatim prefix in a form we do not support (`\\?\something-else`).
    UnsupportedVerbatim,
}

/// Lexically classify `candidate` against the policy for the given target
/// family. Split out from [`require_absolute`] (which uses the compile
/// target) so the full Windows table is unit-testable on Linux CI.
pub fn classify(candidate: &str, windows: bool) -> Result<(), PathRejection> {
    if candidate.is_empty() {
        return Err(PathRejection::Empty);
    }
    if !windows {
        return if candidate.starts_with('/') {
            Ok(())
        } else {
            Err(PathRejection::Relative)
        };
    }

    let bytes = candidate.as_bytes();
    let sep = |b: u8| b == b'\\' || b == b'/';

    // Verbatim namespace: `\\?\C:\…` or `\\?\UNC\server\share…`.
    if let Some(rest) = strip_verbatim_prefix(candidate) {
        if drive_absolute(rest) {
            return Ok(());
        }
        if let Some(unc) = rest
            .strip_prefix("UNC\\")
            .or_else(|| rest.strip_prefix("UNC/"))
        {
            return require_server_and_share(unc);
        }
        return Err(PathRejection::UnsupportedVerbatim);
    }

    // Device namespace: `\\.\COM1` and friends. Never a store directory.
    if candidate.len() >= 4 && sep(bytes[0]) && sep(bytes[1]) && bytes[2] == b'.' && sep(bytes[3]) {
        return Err(PathRejection::DeviceNamespace);
    }

    // UNC: `\\server\share[\…]` — both components required.
    if candidate.len() >= 2 && sep(bytes[0]) && sep(bytes[1]) {
        return require_server_and_share(&candidate[2..]);
    }

    // Drive-letter forms.
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return if drive_absolute(candidate) {
            Ok(())
        } else {
            Err(PathRejection::DriveRelative)
        };
    }

    // Rootful but driveless: `\foo` / `/foo` — relative to the current drive.
    if sep(bytes[0]) {
        return Err(PathRejection::RootRelative);
    }

    Err(PathRejection::Relative)
}

/// `\\?\`-style verbatim prefix (either separator accepted lexically).
fn strip_verbatim_prefix(s: &str) -> Option<&str> {
    let b = s.as_bytes();
    if s.len() >= 4
        && (b[0] == b'\\' || b[0] == b'/')
        && (b[1] == b'\\' || b[1] == b'/')
        && b[2] == b'?'
        && (b[3] == b'\\' || b[3] == b'/')
    {
        Some(&s[4..])
    } else {
        None
    }
}

/// `X:\…` or `X:/…` (drive letter, colon, separator). Bare `X:` is drive-
/// relative and must NOT match.
fn drive_absolute(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

/// After a UNC prefix: require a non-empty server, a separator, and a
/// non-empty share component.
fn require_server_and_share(rest: &str) -> Result<(), PathRejection> {
    let mut parts = rest.split(|c| c == '\\' || c == '/');
    let server = parts.next().unwrap_or("");
    let share = parts.next().unwrap_or("");
    if server.is_empty() || share.is_empty() {
        return Err(PathRejection::IncompleteUnc);
    }
    Ok(())
}

/// Human-actionable explanation for a rejection. `role` names the field in
/// the user's terms ("store directory", "repository location", …).
pub fn rejection_message(role: &str, value: &str, rejection: PathRejection) -> String {
    match rejection {
        PathRejection::Empty => format!(
            "{role}: a directory is required — enter a full absolute path \
             (for example C:\\LoreData on Windows or /home/you/lore)"
        ),
        PathRejection::Relative => format!(
            "{role}: \"{value}\" is a relative path and would resolve against the \
             app's startup directory (which can be C:\\Windows\\System32) — enter a \
             full absolute path such as C:\\LoreData or /home/you/lore"
        ),
        PathRejection::DriveRelative => format!(
            "{role}: \"{value}\" is drive-relative (it depends on that drive's current \
             directory) — add a backslash after the drive letter, for example C:\\LoreData"
        ),
        PathRejection::RootRelative => format!(
            "{role}: \"{value}\" does not name a drive — include the drive letter, \
             for example C:\\LoreData"
        ),
        PathRejection::DeviceNamespace => format!(
            "{role}: \"{value}\" is a Windows device path, not a folder — choose a \
             normal folder such as C:\\LoreData"
        ),
        PathRejection::IncompleteUnc => format!(
            "{role}: \"{value}\" is an incomplete network path — include both server \
             and share, for example \\\\server\\share\\lore"
        ),
        PathRejection::UnsupportedVerbatim => format!(
            "{role}: \"{value}\" uses an unsupported \\\\?\\ form — use a drive path \
             such as \\\\?\\C:\\LoreData, or drop the \\\\?\\ prefix"
        ),
    }
}

/// Validate a user-supplied or persisted lifecycle path. Trims surrounding
/// whitespace, applies [`classify`] for the compile target, and cross-checks
/// with the platform's own [`std::path::Path::is_absolute`] so the two can
/// never disagree in the accepting direction. Returns the validated
/// `PathBuf` on success; fails closed with an actionable [`LoreError`]
/// otherwise.
pub fn require_absolute(raw: &str, role: &str) -> Result<PathBuf, LoreError> {
    let trimmed = raw.trim();
    classify(trimmed, cfg!(windows))
        .map_err(|r| LoreError::CommandFailed(rejection_message(role, trimmed, r)))?;
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        // Unreachable when `classify` and std agree; kept as a hard backstop
        // so a future divergence fails closed rather than open.
        return Err(LoreError::CommandFailed(rejection_message(
            role,
            trimmed,
            PathRejection::Relative,
        )));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accepts(input: &str, windows: bool) {
        assert_eq!(
            classify(input, windows),
            Ok(()),
            "expected accept: {input:?}"
        );
    }

    fn rejects(input: &str, windows: bool, why: PathRejection) {
        assert_eq!(
            classify(input, windows),
            Err(why),
            "expected reject: {input:?}"
        );
    }

    #[test]
    fn windows_accepts_drive_absolute_including_spaces_and_forward_slashes() {
        accepts(r"C:\LoreData", true);
        accepts(r"c:\loredata", true);
        accepts("C:/LoreData", true);
        accepts(r"C:\Lore Data\with spaces", true);
        accepts(r"D:\", true); // drive root: explicit and absolute
    }

    #[test]
    fn windows_accepts_supported_unc_and_verbatim_forms() {
        accepts(r"\\server\share", true);
        accepts(r"\\server\share\lore", true);
        accepts(r"\\?\C:\LoreData", true);
        accepts(r"\\?\UNC\server\share", true);
    }

    #[test]
    fn windows_rejects_every_cwd_dependent_form() {
        rejects("", true, PathRejection::Empty);
        rejects(".", true, PathRejection::Relative);
        rejects("..", true, PathRejection::Relative);
        rejects("lore", true, PathRejection::Relative);
        rejects(r".\lore", true, PathRejection::Relative);
        rejects(r"..\lore", true, PathRejection::Relative);
        rejects("C:foo", true, PathRejection::DriveRelative);
        rejects("C:", true, PathRejection::DriveRelative);
        rejects(r"\foo", true, PathRejection::RootRelative);
        rejects("/foo", true, PathRejection::RootRelative);
    }

    #[test]
    fn windows_rejects_incomplete_unc_and_device_paths() {
        rejects(r"\\", true, PathRejection::IncompleteUnc);
        rejects(r"\\server", true, PathRejection::IncompleteUnc);
        rejects(r"\\server\", true, PathRejection::IncompleteUnc);
        rejects(r"\\.\COM1", true, PathRejection::DeviceNamespace);
        rejects(r"\\?\lore", true, PathRejection::UnsupportedVerbatim);
    }

    #[test]
    fn unix_accepts_only_rooted_paths() {
        accepts("/srv/lore", false);
        accepts("/srv/lore data/with spaces", false);
        accepts("//host/share", false); // still rooted on POSIX
        rejects("", false, PathRejection::Empty);
        rejects(".", false, PathRejection::Relative);
        rejects("..", false, PathRejection::Relative);
        rejects("lore", false, PathRejection::Relative);
        rejects("./lore", false, PathRejection::Relative);
        rejects(r"C:\LoreData", false, PathRejection::Relative); // not a unix root
        rejects("C:foo", false, PathRejection::Relative);
    }

    #[test]
    fn require_absolute_trims_and_returns_the_validated_path() {
        let sample = if cfg!(windows) {
            r"  C:\LoreData  "
        } else {
            "  /srv/lore  "
        };
        let expect = if cfg!(windows) {
            r"C:\LoreData"
        } else {
            "/srv/lore"
        };
        let path = require_absolute(sample, "store directory").expect("absolute path accepted");
        assert_eq!(path, PathBuf::from(expect));
    }

    #[test]
    fn require_absolute_fails_closed_with_an_actionable_message() {
        let err = require_absolute("lore", "store directory").unwrap_err();
        let text = err.to_string();
        assert!(text.contains("store directory"), "names the field: {text}");
        assert!(text.contains("\"lore\""), "echoes the value: {text}");
        assert!(text.contains("absolute path"), "says what to do: {text}");
    }
}
