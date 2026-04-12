use serde::{Deserialize, Serialize};

/// Provider-agnostic normalized representation of a book search result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BookCandidate {
    /// Stable identifier returned to MCP clients for later selection/download.
    pub result_id: String,
    /// Upstream provider kind.
    pub provider: ProviderKind,
    /// Provider-specific item identifier.
    pub provider_item_id: String,
    /// Main title.
    pub title: String,
    /// Optional subtitle or edition information.
    pub subtitle: Option<String>,
    /// Ordered author list.
    pub authors: Vec<String>,
    /// Publisher or imprint when known.
    pub publisher: Option<String>,
    /// Publication year when known.
    pub year: Option<u16>,
    /// Language code or display label.
    pub language: Option<String>,
    /// Normalized file format/extension such as `pdf` or `epub`.
    pub format: Option<String>,
    /// File size in bytes when known.
    pub file_size_bytes: Option<u64>,
    /// Optional cover image URL.
    pub cover_url: Option<String>,
    /// Human-readable source/provider label.
    pub source_label: Option<String>,
    /// MD5 checksum when exposed by the provider.
    pub checksum_md5: Option<String>,
    /// SHA1 checksum when exposed by the provider.
    pub checksum_sha1: Option<String>,
    /// Whether the item is currently downloadable.
    pub downloadable: bool,
    /// Internal ranking score.
    pub relevance_score: f32,
    /// Ranking/debug explanations.
    #[serde(default)]
    pub ranking_reasons: Vec<String>,
    /// Raw provider metadata, only intended for debug flows.
    pub raw_metadata: Option<serde_json::Value>,
}

impl BookCandidate {
    pub fn new(
        result_id: impl Into<String>,
        provider: ProviderKind,
        provider_item_id: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        Self {
            result_id: result_id.into(),
            provider,
            provider_item_id: provider_item_id.into(),
            title: title.into(),
            subtitle: None,
            authors: Vec::new(),
            publisher: None,
            year: None,
            language: None,
            format: None,
            file_size_bytes: None,
            cover_url: None,
            source_label: None,
            checksum_md5: None,
            checksum_sha1: None,
            downloadable: false,
            relevance_score: 0.0,
            ranking_reasons: Vec::new(),
            raw_metadata: None,
        }
    }

    pub fn primary_author(&self) -> Option<&str> {
        self.authors.first().map(String::as_str)
    }

    pub fn display_title(&self) -> String {
        match self.subtitle.as_deref().map(str::trim) {
            Some(subtitle) if !subtitle.is_empty() => format!("{}: {}", self.title, subtitle),
            _ => self.title.clone(),
        }
    }

    pub fn matches_format(&self, desired: &str) -> bool {
        self.format
            .as_deref()
            .map(|actual| actual.eq_ignore_ascii_case(desired))
            .unwrap_or(false)
    }

    pub fn human_summary(&self) -> String {
        let author_text = if self.authors.is_empty() {
            "Unknown author".to_string()
        } else {
            self.authors.join(", ")
        };

        let mut details = Vec::new();

        if let Some(language) = &self.language {
            details.push(language.clone());
        }

        if let Some(year) = self.year {
            details.push(year.to_string());
        }

        if let Some(format) = &self.format {
            details.push(format.to_uppercase());
        }

        if let Some(bytes) = self.file_size_bytes {
            details.push(human_size(bytes));
        }

        if details.is_empty() {
            format!("{} by {}", self.display_title(), author_text)
        } else {
            format!(
                "{} by {} ({})",
                self.display_title(),
                author_text,
                details.join(", ")
            )
        }
    }
}

/// Supported provider identifiers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    AnnasArchive,
    Unknown,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AnnasArchive => "annas_archive",
            Self::Unknown => "unknown",
        }
    }
}

impl Default for ProviderKind {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Known book/file formats.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum BookFormat {
    Pdf,
    Epub,
    Mobi,
    Azw3,
    Djvu,
    Cbz,
    Cbr,
    Txt,
    Rtf,
    Doc,
    Docx,
    Zip,
    Other(String),
}

impl BookFormat {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pdf => "pdf",
            Self::Epub => "epub",
            Self::Mobi => "mobi",
            Self::Azw3 => "azw3",
            Self::Djvu => "djvu",
            Self::Cbz => "cbz",
            Self::Cbr => "cbr",
            Self::Txt => "txt",
            Self::Rtf => "rtf",
            Self::Doc => "doc",
            Self::Docx => "docx",
            Self::Zip => "zip",
            Self::Other(value) => value.as_str(),
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        let normalized = value.trim().trim_start_matches('.').to_ascii_lowercase();
        if normalized.is_empty() {
            return None;
        }

        Some(match normalized.as_str() {
            "pdf" => Self::Pdf,
            "epub" => Self::Epub,
            "mobi" => Self::Mobi,
            "azw3" => Self::Azw3,
            "djvu" => Self::Djvu,
            "cbz" => Self::Cbz,
            "cbr" => Self::Cbr,
            "txt" => Self::Txt,
            "rtf" => Self::Rtf,
            "doc" => Self::Doc,
            "docx" => Self::Docx,
            "zip" => Self::Zip,
            other => Self::Other(other.to_string()),
        })
    }
}

/// Free-text language wrapper.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BookLanguage(String);

impl BookLanguage {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into().trim().to_ascii_lowercase())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Search request model used by the application/service layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchQuery {
    pub query: String,
    pub filters: SearchFilters,
    pub debug: bool,
}

impl SearchQuery {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            filters: SearchFilters::default(),
            debug: false,
        }
    }

    pub fn trimmed_query(&self) -> &str {
        self.query.trim()
    }

    pub fn is_empty(&self) -> bool {
        self.trimmed_query().is_empty()
    }
}

/// Search filter set used for normalized requests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchFilters {
    #[serde(default)]
    pub formats: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    pub limit: Option<u32>,
    pub provider: Option<ProviderKind>,
    pub sort: Option<SearchSort>,
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            formats: Vec::new(),
            categories: Vec::new(),
            languages: Vec::new(),
            year_from: None,
            year_to: None,
            limit: None,
            provider: None,
            sort: Some(SearchSort::Relevance),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchSort {
    Relevance,
    Year,
    Size,
}

/// Additional normalized metadata that can be reused in responses or caching.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BookMetadata {
    pub title: String,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub publisher: Option<String>,
    pub year: Option<u16>,
    pub language: Option<String>,
    pub format: Option<String>,
    pub file_size_bytes: Option<u64>,
    pub cover_url: Option<String>,
    pub source_label: Option<String>,
    pub checksum_md5: Option<String>,
    pub checksum_sha1: Option<String>,
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

    let mut size = bytes as f64;
    let mut unit_index = 0usize;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{size:.1} {}", UNITS[unit_index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_title_uses_subtitle() {
        let mut book = BookCandidate::new(
            "result-1",
            ProviderKind::AnnasArchive,
            "provider-1",
            "The Rust Book",
        );
        book.subtitle = Some("Second Edition".to_string());

        assert_eq!(book.display_title(), "The Rust Book: Second Edition");
    }

    #[test]
    fn primary_author_returns_first_author() {
        let mut book = BookCandidate::new(
            "result-1",
            ProviderKind::AnnasArchive,
            "provider-1",
            "Example",
        );
        book.authors = vec!["Alice".into(), "Bob".into()];

        assert_eq!(book.primary_author(), Some("Alice"));
    }

    #[test]
    fn matches_format_is_case_insensitive() {
        let mut book = BookCandidate::new(
            "result-1",
            ProviderKind::AnnasArchive,
            "provider-1",
            "Example",
        );
        book.format = Some("epub".to_string());

        assert!(book.matches_format("EPUB"));
        assert!(!book.matches_format("pdf"));
    }

    #[test]
    fn human_summary_includes_metadata() {
        let mut book = BookCandidate::new(
            "result-1",
            ProviderKind::AnnasArchive,
            "provider-1",
            "Example",
        );
        book.authors = vec!["Alice".into()];
        book.language = Some("en".into());
        book.year = Some(2024);
        book.format = Some("pdf".into());
        book.file_size_bytes = Some(2 * 1024 * 1024);

        let summary = book.human_summary();

        assert!(summary.contains("Example by Alice"));
        assert!(summary.contains("en"));
        assert!(summary.contains("2024"));
        assert!(summary.contains("PDF"));
        assert!(summary.contains("2.0 MB"));
    }

    #[test]
    fn book_format_parse_handles_known_and_unknown_values() {
        assert_eq!(BookFormat::parse("PDF"), Some(BookFormat::Pdf));
        assert_eq!(
            BookFormat::parse("weird-ext"),
            Some(BookFormat::Other("weird-ext".to_string()))
        );
        assert_eq!(BookFormat::parse(""), None);
    }

    #[test]
    fn search_query_detects_empty_values() {
        let query = SearchQuery::new("   ");
        assert!(query.is_empty());
    }

    #[test]
    fn book_language_normalizes_case() {
        let language = BookLanguage::new("EN");
        assert_eq!(language.as_str(), "en");
    }
}
