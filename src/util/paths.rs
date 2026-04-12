use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use directories::UserDirs;

use crate::errors::{AppError, AppResult};

pub fn default_download_dir() -> AppResult<PathBuf> {
    let user_dirs = UserDirs::new()
        .ok_or_else(|| AppError::config("could not determine user directories".to_string()))?;

    if let Some(download_dir) = user_dirs.download_dir() {
        return Ok(download_dir.to_path_buf());
    }

    Ok(user_dirs.home_dir().join("Downloads"))
}

pub fn resolve_download_dir(explicit: Option<&Path>) -> AppResult<PathBuf> {
    match explicit {
        Some(path) => normalize_user_path(path),
        None => default_download_dir(),
    }
}

pub fn normalize_user_path(path: &Path) -> AppResult<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(AppError::config("path must not be empty".to_string()));
    }

    let expanded = expand_tilde(path)?;
    let normalized = normalize_path(&expanded);

    if normalized.as_os_str().is_empty() {
        return Err(AppError::config(
            "resolved path must not be empty".to_string(),
        ));
    }

    Ok(normalized)
}

pub fn ensure_path_within_root(root: &Path, candidate: &Path) -> AppResult<()> {
    let normalized_root = normalize_path(root);
    let normalized_candidate = normalize_path(candidate);

    if path_starts_with(&normalized_candidate, &normalized_root) {
        Ok(())
    } else {
        Err(AppError::config(format!(
            "resolved path '{}' escapes the allowed root '{}'",
            normalized_candidate.display(),
            normalized_root.display()
        )))
    }
}

pub fn join_filename_safely(root: &Path, filename: &str) -> AppResult<PathBuf> {
    if filename.trim().is_empty() {
        return Err(AppError::config("filename must not be empty".to_string()));
    }

    let file_path = Path::new(filename);

    if file_path.is_absolute() {
        return Err(AppError::config(
            "filename must not be an absolute path".to_string(),
        ));
    }

    if file_path.components().count() != 1 {
        return Err(AppError::config(
            "filename must not contain path separators or traversal components".to_string(),
        ));
    }

    if matches!(
        file_path.components().next(),
        Some(Component::CurDir | Component::ParentDir | Component::RootDir | Component::Prefix(_))
    ) {
        return Err(AppError::config(
            "filename contains invalid path components".to_string(),
        ));
    }

    let joined = root.join(file_path);
    ensure_path_within_root(root, &joined)?;

    Ok(joined)
}

pub fn path_starts_with(path: &Path, base: &Path) -> bool {
    let mut path_components = path.components();
    let mut base_components = base.components();

    loop {
        match base_components.next() {
            Some(base_component) => match path_components.next() {
                Some(path_component) if path_component == base_component => continue,
                _ => return false,
            },
            None => return true,
        }
    }
}

pub fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop = normalized.file_name().is_some()
                    || matches!(
                        normalized.components().next_back(),
                        Some(Component::Normal(_))
                    );

                if can_pop {
                    normalized.pop();
                } else if normalized.as_os_str().is_empty() {
                    normalized.push("..");
                }
            }
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::Normal(part) => normalized.push(part),
        }
    }

    normalized
}

pub fn expand_tilde(path: &Path) -> AppResult<PathBuf> {
    let mut components = path.components();

    let Some(first) = components.next() else {
        return Ok(path.to_path_buf());
    };

    let is_tilde = matches!(first, Component::Normal(part) if part == OsStr::new("~"));
    if !is_tilde {
        return Ok(path.to_path_buf());
    }

    let user_dirs = UserDirs::new()
        .ok_or_else(|| AppError::config("could not determine user home directory".to_string()))?;

    let mut expanded = PathBuf::from(user_dirs.home_dir());

    for component in components {
        expanded.push(component.as_os_str());
    }

    Ok(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_path_removes_current_dir_segments() {
        let normalized = normalize_path(Path::new("./books/./fiction"));

        assert_eq!(normalized, PathBuf::from("books/fiction"));
    }

    #[test]
    fn normalize_path_resolves_parent_segments_when_possible() {
        let normalized = normalize_path(Path::new("books/fiction/../nonfiction"));

        assert_eq!(normalized, PathBuf::from("books/nonfiction"));
    }

    #[test]
    fn join_filename_safely_rejects_nested_paths() {
        let result = join_filename_safely(Path::new("/tmp/downloads"), "nested/file.pdf");

        assert!(result.is_err());
    }

    #[test]
    fn ensure_path_within_root_allows_descendants() {
        let result = ensure_path_within_root(
            Path::new("/tmp/downloads"),
            Path::new("/tmp/downloads/book.pdf"),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn ensure_path_within_root_rejects_escape() {
        let result = ensure_path_within_root(
            Path::new("/tmp/downloads"),
            Path::new("/tmp/other/book.pdf"),
        );

        assert!(result.is_err());
    }
}
