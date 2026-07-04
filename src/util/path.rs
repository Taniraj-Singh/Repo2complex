//! Entry path validation and normalization.
//!
//! Archive entry names are stored as slash-separated paths. Because an archive
//! is untrusted, the reader must never let an entry path escape the extraction
//! root (the classic "zip slip" vulnerability). [`normalize`] rejects absolute
//! paths, `..` traversal, empty components, and NUL bytes, returning a clean,
//! forward-slash path or an error.

use crate::errors::{Error, Result};

/// Validates and normalizes an archive entry path.
///
/// Rules:
/// * must be non-empty,
/// * must not be absolute (`/foo`) or a Windows drive path (`C:\`),
/// * must not contain a `..` component (parent traversal),
/// * must not contain empty components (`a//b`) or `.` components,
/// * must not contain NUL bytes or backslashes.
///
/// Backslashes are rejected rather than translated so that a hostile archive
/// cannot smuggle a separator past a naive consumer.
pub fn normalize(path: &str) -> Result<String> {
    if path.is_empty() {
        return Err(Error::InvalidPath(path.to_owned()));
    }
    if path.starts_with('/') || path.contains('\\') || path.contains('\0') {
        return Err(Error::InvalidPath(path.to_owned()));
    }
    // Reject Windows drive-letter prefixes like `C:`.
    if let Some((first, _)) = path.split_once('/') {
        if is_drive_component(first) {
            return Err(Error::InvalidPath(path.to_owned()));
        }
    } else if is_drive_component(path) {
        return Err(Error::InvalidPath(path.to_owned()));
    }

    let mut out = String::with_capacity(path.len());
    for component in path.split('/') {
        match component {
            "" | "." => return Err(Error::InvalidPath(path.to_owned())),
            ".." => return Err(Error::InvalidPath(path.to_owned())),
            c => {
                if !out.is_empty() {
                    out.push('/');
                }
                out.push_str(c);
            }
        }
    }
    Ok(out)
}

/// Whether normalizing `path` would succeed.
pub fn is_valid(path: &str) -> bool {
    normalize(path).is_ok()
}

/// Splits a normalized path into its components.
pub fn components(path: &str) -> impl Iterator<Item = &str> {
    path.split('/').filter(|c| !c.is_empty())
}

/// Returns the final component (the "file name") of a path.
pub fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Returns the parent portion of a path, or `None` for a top-level name.
pub fn parent(path: &str) -> Option<&str> {
    path.rfind('/').map(|i| &path[..i])
}

fn is_drive_component(c: &str) -> bool {
    let bytes = c.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_clean_paths() {
        assert_eq!(normalize("a/b/c.txt").unwrap(), "a/b/c.txt");
        assert_eq!(normalize("file").unwrap(), "file");
    }

    #[test]
    fn rejects_traversal_and_absolute() {
        assert!(normalize("/etc/passwd").is_err());
        assert!(normalize("../secret").is_err());
        assert!(normalize("a/../../b").is_err());
        assert!(normalize("C:/win").is_err());
        assert!(normalize("a\\b").is_err());
        assert!(normalize("a//b").is_err());
        assert!(normalize("a/./b").is_err());
        assert!(normalize("").is_err());
        assert!(normalize("with\0nul").is_err());
    }

    #[test]
    fn component_helpers() {
        assert_eq!(file_name("a/b/c"), "c");
        assert_eq!(file_name("solo"), "solo");
        assert_eq!(parent("a/b/c"), Some("a/b"));
        assert_eq!(parent("solo"), None);
        let comps: Vec<_> = components("a/b/c").collect();
        assert_eq!(comps, ["a", "b", "c"]);
    }
}
