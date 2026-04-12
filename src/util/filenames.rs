use std::path::{Path, PathBuf};

const DEFAULT_EXTENSION: &str = "bin";
const DEFAULT_MAX_BASENAME_LEN: usize = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilenameParts {
    pub basename: String,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilenameOptions {
    pub max_basename_len: usize,
    pub fallback_stem: String,
    pub fallback_extension: String,
    pub preserve_unicode: bool,
}

impl Default for FilenameOptions {
    fn default() -> Self {
        Self {
            max_basename_len: DEFAULT_MAX_BASENAME_LEN,
            fallback_stem: "download".to_string(),
            fallback_extension: DEFAULT_EXTENSION.to_string(),
            preserve_unicode: true,
        }
    }
}

pub fn build_download_filename(
    title: Option<&str>,
    primary_author: Option<&str>,
    extension: Option<&str>,
) -> String {
    build_download_filename_with_options(
        title,
        primary_author,
        extension,
        &FilenameOptions::default(),
    )
}

pub fn build_download_filename_with_options(
    title: Option<&str>,
    primary_author: Option<&str>,
    extension: Option<&str>,
    options: &FilenameOptions,
) -> String {
    let title = sanitize_component(title.unwrap_or(""), options);
    let author = sanitize_component(primary_author.unwrap_or(""), options);

    let stem = match (title.is_empty(), author.is_empty()) {
        (false, false) => format!("{title} - {author}"),
        (false, true) => title,
        (true, false) => author,
        (true, true) => sanitize_component(&options.fallback_stem, options),
    };

    let stem = trim_basename(&normalize_whitespace(&stem), options.max_basename_len);
    let stem = if stem.is_empty() {
        sanitize_component(&options.fallback_stem, options)
    } else {
        stem
    };

    let ext = normalize_extension(extension)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| options.fallback_extension.clone());

    format!("{stem}.{ext}")
}

pub fn ensure_unique_path(dir: &Path, desired_filename: &str) -> PathBuf {
    let parts = split_filename(desired_filename);
    let base = if parts.basename.is_empty() {
        "download".to_string()
    } else {
        parts.basename
    };

    let extension_suffix = parts
        .extension
        .as_ref()
        .filter(|value| !value.is_empty())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();

    let initial = dir.join(format!("{base}{extension_suffix}"));
    if !initial.exists() {
        return initial;
    }

    for index in 1.. {
        let candidate = dir.join(format!("{base} ({index}){extension_suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("infinite iterator should always yield a filename")
}

pub fn split_filename(filename: &str) -> FilenameParts {
    let trimmed = filename.trim().trim_matches('.');
    if trimmed.is_empty() {
        return FilenameParts {
            basename: String::new(),
            extension: None,
        };
    }

    match trimmed.rsplit_once('.') {
        Some((base, ext)) if !base.is_empty() && !ext.is_empty() => FilenameParts {
            basename: base.to_string(),
            extension: Some(ext.to_ascii_lowercase()),
        },
        _ => FilenameParts {
            basename: trimmed.to_string(),
            extension: None,
        },
    }
}

pub fn normalize_extension(extension: Option<&str>) -> Option<String> {
    extension.and_then(|raw| {
        let trimmed = raw.trim().trim_start_matches('.').trim();
        if trimmed.is_empty() {
            return None;
        }

        let normalized: String = trimmed
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .flat_map(|ch| ch.to_lowercase())
            .collect();

        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    })
}

pub fn sanitize_component(input: &str, options: &FilenameOptions) -> String {
    let mut out = String::with_capacity(input.len());

    for ch in input.chars() {
        if is_forbidden_filename_char(ch) {
            out.push('_');
            continue;
        }

        if ch.is_control() {
            out.push(' ');
            continue;
        }

        if !options.preserve_unicode && !ch.is_ascii() {
            out.push('_');
            continue;
        }

        out.push(ch);
    }

    let normalized = normalize_whitespace(out.trim());
    let normalized = strip_trailing_dots_and_spaces(&normalized);
    let normalized = collapse_repeated_underscores(&normalized);

    if normalized.is_empty() {
        return String::new();
    }

    if is_windows_reserved_name(&normalized) {
        format!("_{normalized}")
    } else {
        normalized
    }
}

pub fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn strip_trailing_dots_and_spaces(input: &str) -> String {
    input.trim_end_matches([' ', '.']).to_string()
}

pub fn trim_basename(input: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }

    let trimmed = input.trim();
    if trimmed.chars().count() <= max_len {
        return trimmed.to_string();
    }

    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars().take(max_len) {
        out.push(ch);
    }

    strip_trailing_dots_and_spaces(&out)
}

pub fn reconcile_extension(
    current_extension: Option<&str>,
    declared_extension: Option<&str>,
    detected_extension: Option<&str>,
) -> Option<String> {
    normalize_extension(detected_extension)
        .or_else(|| normalize_extension(declared_extension))
        .or_else(|| normalize_extension(current_extension))
}

fn collapse_repeated_underscores(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_underscore = false;

    for ch in input.chars() {
        if ch == '_' {
            if !prev_underscore {
                out.push(ch);
            }
            prev_underscore = true;
        } else {
            out.push(ch);
            prev_underscore = false;
        }
    }

    out
}

fn is_forbidden_filename_char(ch: char) -> bool {
    matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
}

fn is_windows_reserved_name(value: &str) -> bool {
    let stem = value
        .split('.')
        .next()
        .map(|part| part.trim())
        .unwrap_or("")
        .to_ascii_uppercase();

    matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_expected_download_filename() {
        let filename = build_download_filename(
            Some("The Pragmatic Programmer"),
            Some("Andrew Hunt"),
            Some("epub"),
        );

        assert_eq!(filename, "The Pragmatic Programmer - Andrew Hunt.epub");
    }

    #[test]
    fn falls_back_when_title_and_author_missing() {
        let filename = build_download_filename(None, None, None);
        assert_eq!(filename, "download.bin");
    }

    #[test]
    fn sanitizes_forbidden_characters() {
        let options = FilenameOptions::default();
        let sanitized = sanitize_component("bad:/\\name*?\"<>|", &options);
        assert_eq!(sanitized, "bad_name_");
    }

    #[test]
    fn prefixes_windows_reserved_names() {
        let options = FilenameOptions::default();
        assert_eq!(sanitize_component("CON", &options), "_CON");
        assert_eq!(sanitize_component("nul", &options), "_nul");
    }

    #[test]
    fn normalizes_extension_values() {
        assert_eq!(normalize_extension(Some(".EPUB")), Some("epub".to_string()));
        assert_eq!(
            normalize_extension(Some(" mobi ")),
            Some("mobi".to_string())
        );
        assert_eq!(normalize_extension(Some("..???")), None);
    }

    #[test]
    fn splits_filename_correctly() {
        let parts = split_filename("example.book.pdf");
        assert_eq!(parts.basename, "example.book");
        assert_eq!(parts.extension, Some("pdf".to_string()));

        let parts = split_filename("README");
        assert_eq!(parts.basename, "README");
        assert_eq!(parts.extension, None);
    }

    #[test]
    fn reconciles_extension_in_priority_order() {
        let extension = reconcile_extension(Some("tmp"), Some("pdf"), Some("epub"));
        assert_eq!(extension, Some("epub".to_string()));
    }

    #[test]
    fn trims_basename_on_char_boundary() {
        let trimmed = trim_basename("abcdefgh", 5);
        assert_eq!(trimmed, "abcde");
    }
}
