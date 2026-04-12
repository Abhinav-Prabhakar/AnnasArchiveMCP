use std::fs;
use std::path::{Path, PathBuf};

use annas_archive_mcp::config::{
    AppConfig, DownloadConfig, LoggingConfig, NetworkConfig, ProviderConfig, ProviderKind,
    RapidApiConfig, SearchConfig, ServerConfig,
};
use annas_archive_mcp::domain::book::{BookCandidate, ProviderKind as DomainProviderKind};
use annas_archive_mcp::domain::download::{DownloadRequest, SelectionMode};
use annas_archive_mcp::domain::ranking::{
    rank_candidates, select_best_candidate, RankingCandidate, RankingPreferences, RankingQuery,
};
use annas_archive_mcp::util::filenames::{
    build_download_filename, ensure_unique_path, normalize_extension, sanitize_component,
};
use annas_archive_mcp::util::formatting::{format_authors, format_bytes, summarize_candidate};
use annas_archive_mcp::util::paths::{
    ensure_path_within_root, join_filename_safely, normalize_path,
};
use tempfile::tempdir;

fn test_config() -> AppConfig {
    AppConfig {
        provider: ProviderConfig {
            default: ProviderKind::AnnasArchive,
        },
        rapidapi: RapidApiConfig {
            key: Some("test-key".to_string()),
            host: "annas-archive-api.p.rapidapi.com".to_string(),
        },
        download: DownloadConfig {
            default_dir: PathBuf::from("."),
            open_after_download_default: false,
            create_subdirs: true,
            overwrite_default: false,
            filename_template: "{title} - {primary_author}.{ext}".to_string(),
            max_filename_length: 180,
        },
        search: SearchConfig {
            default_limit: 10,
            max_limit: 50,
            enable_debug_fields: true,
        },
        network: NetworkConfig {
            timeout_secs: 30,
            max_retries: 3,
            user_agent: "annas-archive-mcp-test/0.1.0".to_string(),
        },
        logging: LoggingConfig {
            level: "info".to_string(),
        },
        server: ServerConfig {
            name: "annas-archive-mcp".to_string(),
            version: "0.1.0".to_string(),
        },
    }
}

fn sample_book(
    id: &str,
    title: &str,
    author: &str,
    format: &str,
    language: &str,
    size: u64,
) -> BookCandidate {
    let mut book = BookCandidate::new(
        id.to_string(),
        DomainProviderKind::AnnasArchive,
        format!("provider-{id}"),
        title.to_string(),
    );
    book.authors = vec![author.to_string()];
    book.format = Some(format.to_string());
    book.language = Some(language.to_string());
    book.file_size_bytes = Some(size);
    book.year = Some(2020);
    book.downloadable = true;
    book
}

fn sample_ranking_candidate(
    id: &str,
    title: &str,
    author: &str,
    format: &str,
    language: &str,
    downloadable: bool,
) -> RankingCandidate {
    RankingCandidate {
        result_id: id.to_string(),
        provider: "annas_archive".to_string(),
        provider_item_id: format!("provider-{id}"),
        title: title.to_string(),
        subtitle: None,
        authors: vec![author.to_string()],
        publisher: Some("Test Publisher".to_string()),
        year: Some(2022),
        language: Some(language.to_string()),
        format: Some(format.to_string()),
        category: Some("nonfiction".to_string()),
        file_size_bytes: Some(5 * 1024 * 1024),
        source_label: Some("libgenRs".to_string()),
        checksum_md5: Some(format!("md5-{id}")),
        downloadable,
    }
}

#[test]
fn config_validation_accepts_good_defaults() {
    let config = test_config();
    assert!(config.validate().is_ok());
    assert_eq!(config.effective_limit(None), 10);
    assert_eq!(config.effective_limit(Some(100)), 50);
}

#[test]
fn config_requires_provider_credentials() {
    let mut config = test_config();
    config.rapidapi.key = None;

    let error = config.require_provider_credentials().unwrap_err();
    assert!(
        error.to_string().to_lowercase().contains("rapidapi")
            || error
                .to_string()
                .to_lowercase()
                .contains("provider credentials")
    );
}

#[test]
fn build_download_filename_uses_title_author_and_extension() {
    let filename = build_download_filename(
        Some("The Rust Programming Language"),
        Some("Steve Klabnik"),
        Some("epub"),
    );

    assert_eq!(
        filename,
        "The Rust Programming Language - Steve Klabnik.epub"
    );
}

#[test]
fn sanitize_component_removes_forbidden_characters() {
    let sanitized = sanitize_component("bad:/\\name*?\"<>|", &Default::default());
    assert!(!sanitized.contains(':'));
    assert!(!sanitized.contains('/'));
    assert!(!sanitized.contains('\\'));
    assert!(!sanitized.contains('*'));
    assert!(!sanitized.contains('?'));
    assert!(!sanitized.contains('"'));
    assert!(!sanitized.contains('<'));
    assert!(!sanitized.contains('>'));
    assert!(!sanitized.contains('|'));
}

#[test]
fn sanitize_component_protects_windows_reserved_names() {
    let sanitized = sanitize_component("CON", &Default::default());
    assert_eq!(sanitized, "_CON");
}

#[test]
fn normalize_extension_cleans_input() {
    assert_eq!(normalize_extension(Some(".EPUB")), Some("epub".to_string()));
    assert_eq!(
        normalize_extension(Some(" mobi ")),
        Some("mobi".to_string())
    );
    assert_eq!(normalize_extension(Some("..???")), None);
}

#[test]
fn resolve_unique_filename_adds_suffix_when_file_exists() {
    let dir = tempdir().unwrap();
    let original = dir.path().join("Book - Author.pdf");
    fs::write(&original, b"existing").unwrap();

    let generated = ensure_unique_path(dir.path(), "Book - Author.pdf");

    assert_eq!(generated, dir.path().join("Book - Author (1).pdf"));
}

#[test]
fn summarize_candidate_includes_core_metadata() {
    let candidate = sample_book(
        "r1",
        "The Rust Book",
        "Ferris Crab",
        "epub",
        "en",
        3 * 1024 * 1024,
    );

    let summary = summarize_candidate(&candidate);

    assert!(summary.contains("The Rust Book"));
    assert!(summary.contains("Ferris Crab"));
    assert!(summary.contains("EPUB"));
    assert!(summary.contains("3.0 MiB"));
    assert!(summary.contains("r1"));
}

#[test]
fn format_authors_formats_multiple_values() {
    let authors = vec!["Alice".to_string(), "Bob".to_string(), "Carol".to_string()];

    assert_eq!(format_authors(&authors), "Alice, Bob and Carol");
}

#[test]
fn format_bytes_uses_binary_units() {
    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(1024), "1.0 KiB");
    assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MiB");
}

#[test]
fn normalize_path_collapses_navigation_segments() {
    let normalized = normalize_path(Path::new("./books/fiction/../rust"));
    assert_eq!(normalized, PathBuf::from("books/rust"));
}

#[test]
fn join_filename_safely_rejects_nested_paths() {
    let result = join_filename_safely(Path::new("/tmp/downloads"), "nested/file.pdf");
    assert!(result.is_err());
}

#[test]
fn ensure_path_within_root_accepts_descendant() {
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

#[test]
fn ranking_prefers_exact_title_and_author_match() {
    let query = RankingQuery {
        raw_query: "Clean Code Robert Martin".to_string(),
        title_hint: Some("Clean Code".to_string()),
        author_hint: Some("Robert Martin".to_string()),
    };

    let good = sample_ranking_candidate("1", "Clean Code", "Robert Martin", "epub", "en", true);
    let weaker =
        sample_ranking_candidate("2", "The Clean Coder", "Robert Martin", "epub", "en", true);

    let ranked = rank_candidates(&query, vec![weaker, good], &RankingPreferences::default());

    assert_eq!(ranked[0].candidate.result_id, "1");
    assert!(ranked[0].breakdown.score > ranked[1].breakdown.score);
}

#[test]
fn ranking_penalizes_non_downloadable_candidates() {
    let query = RankingQuery {
        raw_query: "Dune".to_string(),
        title_hint: None,
        author_hint: None,
    };

    let downloadable = sample_ranking_candidate("1", "Dune", "Frank Herbert", "epub", "en", true);
    let unavailable = sample_ranking_candidate("2", "Dune", "Frank Herbert", "epub", "en", false);

    let ranked = rank_candidates(
        &query,
        vec![unavailable, downloadable],
        &RankingPreferences::default(),
    );

    assert_eq!(ranked[0].candidate.result_id, "1");
}

#[test]
fn ranking_selection_returns_confident_match_when_margin_is_large() {
    let query = RankingQuery {
        raw_query: "The Rust Programming Language".to_string(),
        title_hint: Some("The Rust Programming Language".to_string()),
        author_hint: None,
    };

    let best = sample_ranking_candidate(
        "best",
        "The Rust Programming Language",
        "Steve Klabnik",
        "epub",
        "en",
        true,
    );
    let next =
        sample_ranking_candidate("next", "Programming Rust", "Jim Blandy", "pdf", "en", true);

    let decision = select_best_candidate(
        &query,
        vec![best, next],
        &RankingPreferences::default(),
        5.0,
        15.0,
    );

    assert!(decision.confident);
    assert!(decision.selected.is_some());
    assert_eq!(decision.selected.unwrap().candidate.result_id, "best");
}

#[test]
fn ranking_selection_requires_manual_choice_when_scores_are_close() {
    let query = RankingQuery {
        raw_query: "Atomic Habits".to_string(),
        title_hint: None,
        author_hint: None,
    };

    let first = sample_ranking_candidate("1", "Atomic Habits", "James Clear", "epub", "en", true);
    let second = sample_ranking_candidate(
        "2",
        "Atomic Habits Workbook",
        "James Clear",
        "pdf",
        "en",
        true,
    );

    let decision = select_best_candidate(
        &query,
        vec![first, second],
        &RankingPreferences::default(),
        50.0,
        15.0,
    );

    assert!(!decision.confident);
    assert!(decision.selected.is_none());
    assert_eq!(decision.alternatives.len(), 2);
}

#[test]
fn ranking_query_normalization_removes_extra_spaces() {
    let query = RankingQuery {
        raw_query: "  The   Rust   Book  ".to_string(),
        title_hint: None,
        author_hint: None,
    };

    assert_eq!(query.normalized_query(), "the rust book");
}

#[test]
fn download_request_validation_requires_result_id_for_default_mode() {
    let request = DownloadRequest {
        selection_mode: SelectionMode::ResultId,
        result_id: None,
        provider_item_id: None,
        legacy_index: None,
        download_dir: None,
        filename_template: None,
        overwrite: false,
        create_subdirs: true,
        open_after_download: false,
    };

    assert!(request.validate().is_err());
}

#[test]
fn download_request_validation_accepts_provider_item_id_mode() {
    let request = DownloadRequest {
        selection_mode: SelectionMode::ProviderItemId,
        result_id: None,
        provider_item_id: Some("provider-123".to_string()),
        legacy_index: None,
        download_dir: None,
        filename_template: None,
        overwrite: false,
        create_subdirs: true,
        open_after_download: false,
    };

    assert!(request.validate().is_ok());
}

#[test]
fn download_request_validation_accepts_legacy_index_mode() {
    let request = DownloadRequest {
        selection_mode: SelectionMode::LegacyByIndex,
        result_id: None,
        provider_item_id: None,
        legacy_index: Some(0),
        download_dir: None,
        filename_template: None,
        overwrite: false,
        create_subdirs: true,
        open_after_download: false,
    };

    assert!(request.validate().is_ok());
}

#[test]
fn sample_book_summary_is_stable_for_user_display() {
    let candidate = sample_book(
        "stable",
        "Programming Rust",
        "Jim Blandy",
        "pdf",
        "en",
        10 * 1024 * 1024,
    );

    let summary = candidate.human_summary();

    assert!(summary.contains("Programming Rust"));
    assert!(summary.contains("Jim Blandy"));
    assert!(summary.contains("PDF"));
    assert!(summary.contains("10.0 MB"));
}

#[test]
fn temporary_directory_test_can_create_and_cleanup_partial_file() {
    let dir = tempdir().unwrap();
    let partial = dir.path().join("book.epub.part");

    fs::write(&partial, b"incomplete bytes").unwrap();
    assert!(partial.exists());

    fs::remove_file(&partial).unwrap();
    assert!(!partial.exists());
}
