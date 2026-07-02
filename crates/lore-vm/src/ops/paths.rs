//! Shared path-normalisation helpers for `into_lore` conversions.
//!
//! The lore engine requires absolute paths. Callers that hold a `repo_root`
//! can use these helpers to convert arbitrary path arguments consistently:
//!
//! - **Empty string** — preserved as-is (sentinel for "no path filter / whole
//!   repo" used by diff, status, and similar ops).
//! - **Absolute path** — preserved as-is.
//! - **Non-empty relative path** — joined against `repo_root` to produce an
//!   absolute path.

use lore::interface::{LoreArray, LoreString};

/// Normalise a single path argument for the lore engine.
///
/// # Behaviour
/// | Input               | Output                       |
/// |---------------------|------------------------------|
/// | `""`                | `""` (unchanged sentinel)    |
/// | `"/abs/path"`       | `"/abs/path"` (unchanged)    |
/// | `"relative/file"`   | `"{repo_root}/relative/file"`|
pub(crate) fn lore_path_arg(repo_root: &std::path::Path, value: &str) -> LoreString {
    if value.is_empty() {
        return LoreString::from_str(value);
    }
    let p = std::path::Path::new(value);
    if p.is_absolute() {
        LoreString::from_str(value)
    } else {
        LoreString::from_path(repo_root.join(p))
    }
}

/// Normalise a slice of path arguments for the lore engine.
///
/// Applies [`lore_path_arg`] to each element.
pub(crate) fn lore_path_args(
    repo_root: &std::path::Path,
    values: &[String],
) -> LoreArray<LoreString> {
    LoreArray::from_vec(values.iter().map(|v| lore_path_arg(repo_root, v)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = "/repo/root";

    fn root() -> &'static std::path::Path {
        std::path::Path::new(ROOT)
    }

    // ── lore_path_arg ──────────────────────────────────────────────────────

    #[test]
    fn empty_string_is_preserved() {
        let result = lore_path_arg(root(), "");
        assert_eq!(
            result.as_str(),
            "",
            "empty string must pass through unchanged"
        );
    }

    #[test]
    fn absolute_path_is_preserved() {
        let result = lore_path_arg(root(), "/abs/some/file.txt");
        assert_eq!(result.as_str(), "/abs/some/file.txt");
    }

    #[test]
    fn relative_path_is_joined_to_repo_root() {
        let result = lore_path_arg(root(), "src/main.rs");
        assert_eq!(result.as_str(), "/repo/root/src/main.rs");
    }

    #[test]
    fn relative_nested_path_is_joined() {
        let result = lore_path_arg(root(), "a/b/c.txt");
        assert_eq!(result.as_str(), "/repo/root/a/b/c.txt");
    }

    #[test]
    fn single_filename_is_joined() {
        let result = lore_path_arg(root(), "file.txt");
        assert_eq!(result.as_str(), "/repo/root/file.txt");
    }

    // ── lore_path_args ─────────────────────────────────────────────────────

    #[test]
    fn empty_vec_returns_empty_array() {
        let result = lore_path_args(root(), &[]);
        assert_eq!(result.as_slice().len(), 0);
    }

    #[test]
    fn vec_with_empty_string_preserves_it() {
        let values = vec![String::new()];
        let result = lore_path_args(root(), &values);
        let slice = result.as_slice();
        assert_eq!(slice.len(), 1);
        assert_eq!(slice[0].as_str(), "");
    }

    #[test]
    fn vec_with_absolute_path_preserves_it() {
        let values = vec!["/abs/path.txt".to_string()];
        let result = lore_path_args(root(), &values);
        let slice = result.as_slice();
        assert_eq!(slice[0].as_str(), "/abs/path.txt");
    }

    #[test]
    fn vec_with_relative_path_joins_to_root() {
        let values = vec!["rel/path.txt".to_string()];
        let result = lore_path_args(root(), &values);
        let slice = result.as_slice();
        assert_eq!(slice[0].as_str(), "/repo/root/rel/path.txt");
    }

    #[test]
    fn vec_mixed_empty_absolute_relative() {
        let values = vec![
            String::new(),
            "/abs/file".to_string(),
            "rel/file".to_string(),
        ];
        let result = lore_path_args(root(), &values);
        let slice = result.as_slice();
        assert_eq!(slice.len(), 3);
        assert_eq!(slice[0].as_str(), "");
        assert_eq!(slice[1].as_str(), "/abs/file");
        assert_eq!(slice[2].as_str(), "/repo/root/rel/file");
    }
}
