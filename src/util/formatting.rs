use crate::domain::book::BookCandidate;

/// Format a byte count into a human-readable string using binary units.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];

    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let mut value = bytes as f64;
    let mut unit_index = 0usize;

    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }

    format!("{value:.1} {}", UNITS[unit_index])
}

/// Join a list of authors into a user-friendly string.
pub fn format_authors(authors: &[String]) -> String {
    match authors {
        [] => "Unknown author".to_string(),
        [one] => one.clone(),
        [first, second] => format!("{first}, {second}"),
        _ => {
            let head = &authors[..authors.len() - 1];
            let tail = &authors[authors.len() - 1];
            format!("{} and {}", head.join(", "), tail)
        }
    }
}

/// Build a concise one-line summary for a search result candidate.
pub fn summarize_candidate(candidate: &BookCandidate) -> String {
    let authors = format_authors(&candidate.authors);
    let year = candidate
        .year
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".to_string());
    let language = candidate
        .language
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("unknown");
    let file_format = candidate
        .format
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_uppercase())
        .unwrap_or_else(|| "unknown".to_string());
    let size = candidate
        .file_size_bytes
        .map(format_bytes)
        .unwrap_or_else(|| "unknown size".to_string());

    format!(
        "{} by {} ({}, {}, {}, {}) [{}]",
        candidate.title, authors, year, language, file_format, size, candidate.result_id
    )
}

/// Build a multiline summary for a list of candidates.
pub fn summarize_candidates(candidates: &[BookCandidate]) -> String {
    if candidates.is_empty() {
        return "No results found.".to_string();
    }

    candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| format!("{}. {}", index + 1, summarize_candidate(candidate)))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::book::{BookCandidate, ProviderKind};

    fn sample_candidate() -> BookCandidate {
        let mut candidate = BookCandidate::new(
            "result-1",
            ProviderKind::AnnasArchive,
            "provider-1",
            "The Rust Programming Language",
        );
        candidate.authors = vec!["Steve Klabnik".into(), "Carol Nichols".into()];
        candidate.year = Some(2023);
        candidate.language = Some("en".into());
        candidate.format = Some("epub".into());
        candidate.file_size_bytes = Some(5 * 1024 * 1024);
        candidate
    }

    #[test]
    fn formats_small_bytes() {
        assert_eq!(format_bytes(512), "512 B");
    }

    #[test]
    fn formats_large_bytes() {
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MiB");
    }

    #[test]
    fn formats_authors_list() {
        assert_eq!(format_authors(&[]), "Unknown author");
        assert_eq!(format_authors(&["A".into()]), "A");
        assert_eq!(format_authors(&["A".into(), "B".into()]), "A, B");
        assert_eq!(
            format_authors(&["A".into(), "B".into(), "C".into()]),
            "A, B and C"
        );
    }

    #[test]
    fn summarizes_candidate() {
        let summary = summarize_candidate(&sample_candidate());
        assert!(summary.contains("The Rust Programming Language"));
        assert!(summary.contains("Steve Klabnik"));
        assert!(summary.contains("2023"));
        assert!(summary.contains("EPUB"));
    }

    #[test]
    fn summarizes_multiple_candidates() {
        let candidates = vec![sample_candidate(), sample_candidate()];
        let summary = summarize_candidates(&candidates);
        assert!(summary.contains("1."));
        assert!(summary.contains("2."));
    }
}
