use crate::config::AppConfig;
use crate::domain::book::BookCandidate;
use crate::domain::provider::{BookProvider, ProviderSearchRequest, SearchSort};
use crate::domain::ranking::{
    rank_candidates, select_best_candidate, RankedCandidate, RankingCandidate, RankingPreferences,
    RankingQuery, SelectionDecision,
};
use crate::errors::{AppError, AppResult, ValidationError};
use crate::util::formatting::summarize_candidates;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub query: String,
    pub formats: Vec<String>,
    pub categories: Vec<String>,
    pub languages: Vec<String>,
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    pub limit: Option<usize>,
    pub sort: Option<String>,
    pub debug: bool,
}

impl Default for SearchRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            formats: Vec::new(),
            categories: Vec::new(),
            languages: Vec::new(),
            year_from: None,
            year_to: None,
            limit: None,
            sort: None,
            debug: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchResponse {
    pub query: String,
    pub count: usize,
    pub results: Vec<BookCandidate>,
    pub summary: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoSelectionMode {
    Manual,
    AutoIfConfident,
    ForceTop,
}

pub struct SearchService<P: BookProvider> {
    provider: Arc<P>,
    config: Arc<AppConfig>,
}

impl<P: BookProvider> SearchService<P> {
    pub fn new(provider: Arc<P>, config: Arc<AppConfig>) -> Self {
        Self { provider, config }
    }

    pub async fn search(&self, request: SearchRequest) -> AppResult<SearchResponse> {
        self.validate_request(&request)?;

        let normalized = self.normalize_request(request)?;
        let provider_request = ProviderSearchRequest {
            query: normalized.query.clone(),
            formats: normalized.formats.clone(),
            categories: normalized.categories.clone(),
            languages: normalized.languages.clone(),
            year_from: normalized.year_from,
            year_to: normalized.year_to,
            limit: self.config.effective_limit(normalized.limit) as u32,
            sort: parse_sort(normalized.sort.as_deref()),
            debug: normalized.debug,
        };

        let provider_response = self.provider.search(provider_request).await?;

        let ranking_query = RankingQuery {
            raw_query: normalized.query.clone(),
            title_hint: extract_title_hint(&normalized.query),
            author_hint: extract_author_hint(&normalized.query),
        };

        let preferences = build_preferences(&normalized);
        let ranked = rank_candidates(
            &ranking_query,
            provider_response
                .items
                .iter()
                .map(map_book_to_ranking_candidate)
                .collect::<Vec<_>>(),
            &preferences,
        );

        let mut results_by_id = provider_response
            .items
            .into_iter()
            .map(|book| (book.result_id.clone(), book))
            .collect::<std::collections::HashMap<_, _>>();

        let mut results = Vec::new();
        for ranked_candidate in ranked {
            if let Some(mut candidate) = results_by_id.remove(&ranked_candidate.candidate.result_id)
            {
                candidate.relevance_score = ranked_candidate.breakdown.score;
                candidate.ranking_reasons = ranked_candidate.breakdown.reasons;
                if !normalized.debug {
                    candidate.raw_metadata = None;
                }
                results.push(candidate);
            }
        }

        let summary = if results.is_empty() {
            format!("No books matched \"{}\".", normalized.query)
        } else {
            format!(
                "Found {} result(s) for \"{}\".\n{}",
                results.len(),
                normalized.query,
                summarize_candidates(&results)
            )
        };

        Ok(SearchResponse {
            query: normalized.query,
            count: results.len(),
            results,
            summary,
            warnings: provider_response.warnings,
        })
    }

    pub async fn select_candidate(
        &self,
        response: &SearchResponse,
        mode: AutoSelectionMode,
    ) -> AppResult<Option<BookCandidate>> {
        if response.results.is_empty() {
            return Ok(None);
        }

        match mode {
            AutoSelectionMode::Manual => Ok(None),
            AutoSelectionMode::ForceTop => Ok(response.results.first().cloned()),
            AutoSelectionMode::AutoIfConfident => {
                let ranking_query = RankingQuery {
                    raw_query: response.query.clone(),
                    title_hint: extract_title_hint(&response.query),
                    author_hint: extract_author_hint(&response.query),
                };

                let preferences = RankingPreferences::default();
                let decision = select_best_candidate(
                    &ranking_query,
                    response
                        .results
                        .iter()
                        .map(map_book_to_ranking_candidate)
                        .collect::<Vec<_>>(),
                    &preferences,
                    5.0,
                    15.0,
                );

                self.resolve_selection_decision(response, decision)
            }
        }
    }

    pub fn find_by_result_id(
        &self,
        response: &SearchResponse,
        result_id: &str,
    ) -> Option<BookCandidate> {
        response
            .results
            .iter()
            .find(|item| item.result_id == result_id)
            .cloned()
    }

    fn resolve_selection_decision(
        &self,
        response: &SearchResponse,
        decision: SelectionDecision,
    ) -> AppResult<Option<BookCandidate>> {
        if !decision.confident {
            return Ok(None);
        }

        let selected: RankedCandidate = match decision.selected {
            Some(value) => value,
            None => return Ok(None),
        };

        Ok(response
            .results
            .iter()
            .find(|candidate| candidate.result_id == selected.candidate.result_id)
            .cloned())
    }

    fn validate_request(&self, request: &SearchRequest) -> AppResult<()> {
        if request.query.trim().is_empty() {
            return Err(AppError::Validation(ValidationError::EmptyQuery));
        }

        if let (Some(from), Some(to)) = (request.year_from, request.year_to) {
            if from > to {
                return Err(AppError::Validation(ValidationError::Invalid {
                    field: "year_range",
                    reason: "`year_from` must be less than or equal to `year_to`".to_string(),
                }));
            }
        }

        if let Some(limit) = request.limit {
            if limit == 0 {
                return Err(AppError::Validation(ValidationError::Invalid {
                    field: "limit",
                    reason: "limit must be greater than 0".to_string(),
                }));
            }

            if limit > self.config.search.max_limit {
                return Err(AppError::Validation(ValidationError::LimitTooLarge {
                    requested: limit,
                    max: self.config.search.max_limit,
                }));
            }
        }

        Ok(())
    }

    fn normalize_request(&self, request: SearchRequest) -> AppResult<SearchRequest> {
        Ok(SearchRequest {
            query: collapse_whitespace(&request.query),
            formats: normalize_string_list(request.formats),
            categories: normalize_string_list(request.categories),
            languages: normalize_string_list(request.languages),
            year_from: request.year_from,
            year_to: request.year_to,
            limit: Some(self.config.effective_limit(request.limit)),
            sort: request.sort.map(|value| value.trim().to_ascii_lowercase()),
            debug: request.debug || self.config.search.enable_debug_fields,
        })
    }
}

fn build_preferences(request: &SearchRequest) -> RankingPreferences {
    let mut preferences = RankingPreferences::default();

    if !request.formats.is_empty() {
        preferences.preferred_formats = request
            .formats
            .iter()
            .map(|value| crate::domain::ranking::RankingFormat::parse(value))
            .collect();
    }

    if !request.languages.is_empty() {
        preferences.preferred_languages = request.languages.clone();
    }

    if !request.categories.is_empty() {
        preferences.preferred_categories = request
            .categories
            .iter()
            .map(|value| crate::domain::ranking::RankingCategory::parse(value))
            .collect();
    }

    preferences.minimum_year = request.year_from;
    preferences.maximum_year = request.year_to;
    preferences
}

fn map_book_to_ranking_candidate(book: &BookCandidate) -> RankingCandidate {
    RankingCandidate {
        result_id: book.result_id.clone(),
        provider: book.provider.as_str().to_string(),
        provider_item_id: book.provider_item_id.clone(),
        title: book.title.clone(),
        subtitle: book.subtitle.clone(),
        authors: book.authors.clone(),
        publisher: book.publisher.clone(),
        year: book.year,
        language: book.language.clone(),
        format: book.format.clone(),
        category: None,
        file_size_bytes: book.file_size_bytes,
        source_label: book.source_label.clone(),
        checksum_md5: book.checksum_md5.clone(),
        downloadable: book.downloadable,
    }
}

fn parse_sort(value: Option<&str>) -> SearchSort {
    match value.unwrap_or("relevance") {
        "year" => SearchSort::Year,
        "size" => SearchSort::Size,
        _ => SearchSort::Relevance,
    }
}

fn normalize_string_list(values: Vec<String>) -> Vec<String> {
    let mut items = values
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    items.sort();
    items.dedup();
    items
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_title_hint(query: &str) -> Option<String> {
    let query = collapse_whitespace(query);
    if query.is_empty() {
        return None;
    }

    if let Some((title, _author)) = query.split_once(" by ") {
        let title = title.trim();
        if !title.is_empty() {
            return Some(title.to_string());
        }
    }

    Some(query)
}

fn extract_author_hint(query: &str) -> Option<String> {
    let query = collapse_whitespace(query);
    let (_, author) = query.split_once(" by ")?;
    let author = author.trim();
    if author.is_empty() {
        None
    } else {
        Some(author.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        DownloadConfig, LoggingConfig, NetworkConfig, ProviderConfig,
        ProviderKind as ConfigProviderKind, RapidApiConfig, SearchConfig, ServerConfig,
    };
    use crate::domain::book::{BookCandidate, ProviderKind as BookProviderKind};
    use crate::domain::provider::{
        BookProvider, ProviderCapabilities, ProviderDownloadDescriptor, ProviderDownloadRequest,
        ProviderItem, ProviderItemLookupRequest, ProviderKind, ProviderSearchRequest,
        ProviderSearchResponse,
    };
    use async_trait::async_trait;
    use std::path::PathBuf;

    struct FakeProvider {
        items: Vec<BookCandidate>,
    }

    #[async_trait]
    impl BookProvider for FakeProvider {
        fn kind(&self) -> ProviderKind {
            ProviderKind::AnnasArchive
        }

        fn name(&self) -> &'static str {
            "fake"
        }

        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities::default()
        }

        async fn search(
            &self,
            _request: ProviderSearchRequest,
        ) -> Result<ProviderSearchResponse, AppError> {
            Ok(ProviderSearchResponse {
                provider: ProviderKind::AnnasArchive,
                items: self.items.clone(),
                total_returned: self.items.len(),
                warnings: Vec::new(),
                debug_metadata: None,
            })
        }

        async fn get_item(
            &self,
            _request: ProviderItemLookupRequest,
        ) -> Result<ProviderItem, AppError> {
            unreachable!()
        }

        async fn download(
            &self,
            _request: ProviderDownloadRequest,
        ) -> Result<ProviderDownloadDescriptor, AppError> {
            unreachable!()
        }
    }

    fn config() -> Arc<AppConfig> {
        Arc::new(AppConfig {
            provider: ProviderConfig {
                default: ConfigProviderKind::AnnasArchive,
            },
            rapidapi: RapidApiConfig {
                key: Some("test".to_string()),
                host: "example.com".to_string(),
            },
            download: DownloadConfig {
                default_dir: PathBuf::from("."),
                open_after_download_default: false,
                create_subdirs: true,
                overwrite_default: false,
                filename_template: "{title}.{ext}".to_string(),
                max_filename_length: 128,
            },
            search: SearchConfig {
                default_limit: 10,
                max_limit: 50,
                enable_debug_fields: false,
            },
            network: NetworkConfig {
                timeout_secs: 30,
                max_retries: 3,
                user_agent: "test".to_string(),
            },
            logging: LoggingConfig {
                level: "info".to_string(),
            },
            server: ServerConfig {
                name: "libgen-mcp".to_string(),
                version: "0.1.0".to_string(),
            },
        })
    }

    fn candidate(id: &str, title: &str) -> BookCandidate {
        let mut item = BookCandidate::new(
            id.to_string(),
            BookProviderKind::AnnasArchive,
            id.to_string(),
            title,
        );
        item.authors = vec!["Author".to_string()];
        item.format = Some("epub".to_string());
        item.language = Some("en".to_string());
        item.downloadable = true;
        item
    }

    #[tokio::test]
    async fn search_returns_ranked_results() {
        let provider = Arc::new(FakeProvider {
            items: vec![
                candidate("2", "The Rust Programming Language"),
                candidate("1", "Rust Book"),
            ],
        });
        let service = SearchService::new(provider, config());

        let response = service
            .search(SearchRequest {
                query: "The Rust Programming Language".to_string(),
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(response.count, 2);
        assert_eq!(response.results[0].title, "The Rust Programming Language");
    }

    #[test]
    fn author_hint_is_extracted() {
        assert_eq!(
            extract_author_hint("Dune by Frank Herbert"),
            Some("Frank Herbert".to_string())
        );
        assert_eq!(extract_author_hint("Dune"), None);
    }

    #[test]
    fn whitespace_is_collapsed() {
        assert_eq!(collapse_whitespace("  a   b   c "), "a b c");
    }
}
