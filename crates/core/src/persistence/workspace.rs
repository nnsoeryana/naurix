use super::DbError;
use std::path::{Path, PathBuf};

/// Validate a workspace root: must be an absolute path to an existing directory. Returns the
/// canonical path (symlinks resolved) so duplicates are detected reliably.
pub fn validate_root(path: &str) -> Result<PathBuf, DbError> {
    if path.is_empty() || path.contains('\0') {
        return Err(DbError::Invalid("path is empty or contains NUL".into()));
    }
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(DbError::Invalid("path must be absolute".into()));
    }
    let canon = std::fs::canonicalize(p)
        .map_err(|_| DbError::Invalid(format!("directory does not exist: {path}")))?;
    if !canon.is_dir() {
        return Err(DbError::Invalid(format!("not a directory: {path}")));
    }
    Ok(canon)
}

/// Names/titles: 1..=100 chars after trimming, no control characters.
pub fn validate_name(name: &str) -> Result<String, DbError> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 100 || n.chars().any(char::is_control) {
        return Err(DbError::Invalid(
            "name must be 1-100 characters without control characters".into(),
        ));
    }
    Ok(n.to_string())
}
