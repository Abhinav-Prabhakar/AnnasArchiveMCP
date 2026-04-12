use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::AppConfig;
use crate::domain::book::BookCandidate;
use crate::domain::download::{DownloadRequest, DownloadResult, SelectionMode};
use crate::errors::{AppError, AppResult};
use crate::mcp::server::{
    array_schema, boolean_schema, enum_string_schema, integer_schema, json_schema_for_object,
    string_schema, ToolCallResult, ToolContent, ToolDescriptor,
};
use crate::providers::annas_archive::AnnasArchiveProvider;
use crate::services::download_service::DownloadService;
use crate::services::search_service::{
    AutoSelectionMode, SearchRequest, SearchResponse, SearchService,
};

#[derive(Clone)]
pub struct ToolRouter {
    search_service: Arc<SearchService<AnnasArchiveProvider>>,
    download_service: Arc<DownloadService<AnnasArchiveProvider>>,
    config: Arc<AppConfig>,
    descriptors: Vec<ToolDescriptor>,
}

impl ToolRouter {
    pub fn new(
        search_service: Arc<SearchService<AnnasArchiveProvider>>,
        download_service: Arc<DownloadService<AnnasArchiveProvider>>,
        config: Arc<AppConfig>,
    ) -> Self {
        Self {
            search_service,
            download_service,
            config,
            descriptors: vec![
                search_books_descriptor(),
                download_book_descriptor(),
                search_and_download_book_descriptor(),
            ],
        }
    }

    pub fn descriptors(&self) -> Vec<ToolDescriptor> {
        self.descriptors.clone()
    }

    pub async fn call(&self, name: &str, arguments: Value) -> AppResult<ToolCallResult> {
        match name {
            "search_books" => self.handle_search_books(arguments).await,
            "download_book" => self.handle_download_book(arguments).await,
            "search_and_download_book" => self.handle_search_and_download_book(arguments).await,
            _ => Err(AppError::method_not_found(format!("Unknown tool: {name}"))),
        }
    }

    async fn handle_search_books(&self, arguments: Value) -> AppResult<ToolCallResult> {
        let input: SearchBooksInput = parse_arguments(arguments)?;

        let request = SearchRequest {
            query: input.query,
            formats: input.formats,
            categories: input.categories,
            languages: input.languages,
            year_from: input.year_from,
            year_to: input.year_to,
            limit: input
                .limit
                .map(|value| value as usize)
                .or(Some(self.config.search.default_limit)),
            sort: input.sort,
            debug: input.debug,
        };

        let response = self.search_service.search(request).await?;
        let structured = serialize_search_response(&response, input.debug);

        Ok(success_result(response.summary.clone(), structured))
    }

    async fn handle_download_book(&self, arguments: Value) -> AppResult<ToolCallResult> {
        let input: DownloadBookInput = parse_arguments(arguments)?;
        let request = build_download_request(&self.config, &input)?;
        let result = self
            .resolve_and_download(
                &request,
                input.result_id.as_deref(),
                input.provider_item_id.as_deref(),
            )
            .await?;

        Ok(success_result(
            result.summary(),
            serialize_download_result(&result),
        ))
    }

    async fn handle_search_and_download_book(&self, arguments: Value) -> AppResult<ToolCallResult> {
        let input: SearchAndDownloadBookInput = parse_arguments(arguments)?;
        let selection_mode = parse_selection_mode(input.selection_mode.as_deref())?;

        let search_request = SearchRequest {
            query: input.query.clone(),
            formats: input.formats.clone(),
            categories: input.categories.clone(),
            languages: input.languages.clone(),
            year_from: input.year_from,
            year_to: input.year_to,
            limit: input
                .limit
                .map(|value| value as usize)
                .or(Some(self.config.search.default_limit)),
            sort: input.sort.clone(),
            debug: input.debug,
        };

        let search_response = self.search_service.search(search_request).await?;

        if search_response.results.is_empty() {
            let summary = format!("No books matched \"{}\".", input.query);
            return Ok(success_result(
                summary.clone(),
                json!({
                    "summary": summary,
                    "selection_required": false,
                    "query": input.query,
                    "count": 0,
                    "results": [],
                    "warnings": ["No books matched the query and filters provided."]
                }),
            ));
        }

        let explicit_candidate = if let Some(result_id) = input.result_id.as_deref() {
            self.search_service
                .find_by_result_id(&search_response, result_id)
        } else if let Some(provider_item_id) = input.provider_item_id.as_deref() {
            search_response
                .results
                .iter()
                .find(|candidate| candidate.provider_item_id == provider_item_id)
                .cloned()
        } else {
            None
        };

        let selected = match explicit_candidate {
            Some(candidate) => Some(candidate),
            None => {
                self.search_service
                    .select_candidate(&search_response, selection_mode)
                    .await?
            }
        };

        let Some(candidate) = selected else {
            let summary = format!(
                "Found {} plausible matches for \"{}\". Select one with its result_id using download_book.",
                search_response.results.len(),
                search_response.query
            );

            return Ok(success_result(
                summary.clone(),
                json!({
                    "summary": summary,
                    "selection_required": true,
                    "query": search_response.query,
                    "count": search_response.count,
                    "results": search_response
                        .results
                        .iter()
                        .map(|item| serialize_candidate(item, input.debug))
                        .collect::<Vec<_>>(),
                    "warnings": search_response.warnings,
                }),
            ));
        };

        let download_request = DownloadRequest {
            selection_mode: match selection_mode {
                AutoSelectionMode::Manual => {
                    if input.provider_item_id.is_some() {
                        SelectionMode::ProviderItemId
                    } else {
                        SelectionMode::ResultId
                    }
                }
                AutoSelectionMode::AutoIfConfident => SelectionMode::AutoIfConfident,
                AutoSelectionMode::ForceTop => SelectionMode::ForceTop,
            },
            result_id: input
                .result_id
                .clone()
                .or_else(|| Some(candidate.result_id.clone())),
            provider_item_id: input
                .provider_item_id
                .clone()
                .or_else(|| Some(candidate.provider_item_id.clone())),
            legacy_index: None,
            download_dir: input.download_dir.clone().map(std::path::PathBuf::from),
            filename_template: input
                .filename_template
                .clone()
                .or_else(|| Some(self.config.download.filename_template.clone())),
            overwrite: input
                .overwrite
                .unwrap_or(self.config.download.overwrite_default),
            create_subdirs: input
                .create_subdirs
                .unwrap_or(self.config.download.create_subdirs),
            open_after_download: input
                .open_after_download
                .unwrap_or(self.config.download.open_after_download_default),
        };

        let result = self
            .download_service
            .download(&candidate, download_request)
            .await?;
        let summary = result.summary();

        Ok(success_result(
            summary.clone(),
            json!({
                "summary": summary,
                "selection_required": false,
                "search": serialize_search_response(&search_response, input.debug),
                "download": serialize_download_result(&result),
            }),
        ))
    }

    async fn resolve_and_download(
        &self,
        request: &DownloadRequest,
        result_id: Option<&str>,
        provider_item_id: Option<&str>,
    ) -> AppResult<DownloadResult> {
        let candidate = if let Some(result_id) = result_id {
            self.lookup_candidate_by_result_id(result_id).await?
        } else if let Some(provider_item_id) = provider_item_id {
            self.lookup_candidate_by_provider_item_id(provider_item_id)
                .await?
        } else {
            return Err(AppError::validation(
                "Either result_id or provider_item_id must be provided.",
            ));
        };

        self.download_service
            .download(&candidate, request.clone())
            .await
    }

    async fn lookup_candidate_by_result_id(&self, result_id: &str) -> AppResult<BookCandidate> {
        let response = self
            .search_service
            .search(SearchRequest {
                query: result_id.to_string(),
                limit: Some(self.config.search.default_limit),
                debug: true,
                ..Default::default()
            })
            .await?;

        response
            .results
            .into_iter()
            .find(|candidate| candidate.result_id == result_id)
            .ok_or_else(|| AppError::validation(format!("Result id not found: {result_id}")))
    }

    async fn lookup_candidate_by_provider_item_id(
        &self,
        provider_item_id: &str,
    ) -> AppResult<BookCandidate> {
        let response = self
            .search_service
            .search(SearchRequest {
                query: provider_item_id.to_string(),
                limit: Some(self.config.search.default_limit),
                debug: true,
                ..Default::default()
            })
            .await?;

        response
            .results
            .into_iter()
            .find(|candidate| candidate.provider_item_id == provider_item_id)
            .ok_or_else(|| {
                AppError::validation(format!(
                    "Provider item id not found in searchable results: {provider_item_id}"
                ))
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchBooksInput {
    pub query: String,
    #[serde(default)]
    pub formats: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub year_from: Option<u16>,
    #[serde(default)]
    pub year_to: Option<u16>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub debug: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadBookInput {
    #[serde(default)]
    pub result_id: Option<String>,
    #[serde(default)]
    pub provider_item_id: Option<String>,
    #[serde(default)]
    pub download_dir: Option<String>,
    #[serde(default)]
    pub filename_template: Option<String>,
    #[serde(default)]
    pub overwrite: Option<bool>,
    #[serde(default)]
    pub open_after_download: Option<bool>,
    #[serde(default)]
    pub create_subdirs: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchAndDownloadBookInput {
    pub query: String,
    #[serde(default)]
    pub formats: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub year_from: Option<u16>,
    #[serde(default)]
    pub year_to: Option<u16>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub debug: bool,
    #[serde(default)]
    pub result_id: Option<String>,
    #[serde(default)]
    pub provider_item_id: Option<String>,
    #[serde(default)]
    pub selection_mode: Option<String>,
    #[serde(default)]
    pub download_dir: Option<String>,
    #[serde(default)]
    pub filename_template: Option<String>,
    #[serde(default)]
    pub overwrite: Option<bool>,
    #[serde(default)]
    pub open_after_download: Option<bool>,
    #[serde(default)]
    pub create_subdirs: Option<bool>,
}

fn parse_arguments<T>(arguments: Value) -> AppResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(arguments)
        .map_err(|error| AppError::invalid_params(format!("Invalid tool arguments: {error}")))
}

fn build_download_request(
    config: &AppConfig,
    input: &DownloadBookInput,
) -> AppResult<DownloadRequest> {
    if input.result_id.is_none() && input.provider_item_id.is_none() {
        return Err(AppError::validation(
            "Either result_id or provider_item_id must be provided.",
        ));
    }

    Ok(DownloadRequest {
        selection_mode: if input.result_id.is_some() {
            SelectionMode::ResultId
        } else {
            SelectionMode::ProviderItemId
        },
        result_id: input.result_id.clone(),
        provider_item_id: input.provider_item_id.clone(),
        legacy_index: None,
        download_dir: input.download_dir.clone().map(std::path::PathBuf::from),
        filename_template: input
            .filename_template
            .clone()
            .or_else(|| Some(config.download.filename_template.clone())),
        overwrite: input.overwrite.unwrap_or(config.download.overwrite_default),
        create_subdirs: input
            .create_subdirs
            .unwrap_or(config.download.create_subdirs),
        open_after_download: input
            .open_after_download
            .unwrap_or(config.download.open_after_download_default),
    })
}

fn parse_selection_mode(value: Option<&str>) -> AppResult<AutoSelectionMode> {
    match value
        .unwrap_or("auto_if_confident")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "manual" => Ok(AutoSelectionMode::Manual),
        "auto_if_confident" => Ok(AutoSelectionMode::AutoIfConfident),
        "force_top" => Ok(AutoSelectionMode::ForceTop),
        other => Err(AppError::validation(format!(
            "Unsupported selection_mode value: {other}"
        ))),
    }
}

fn success_result(summary: String, structured_content: Value) -> ToolCallResult {
    ToolCallResult {
        content: vec![ToolContent {
            kind: "text".to_string(),
            text: summary,
        }],
        is_error: false,
        structured_content: Some(structured_content),
    }
}

fn serialize_search_response(response: &SearchResponse, debug: bool) -> Value {
    json!({
        "summary": response.summary,
        "query": response.query,
        "count": response.count,
        "results": response
            .results
            .iter()
            .map(|item| serialize_candidate(item, debug))
            .collect::<Vec<_>>(),
        "warnings": response.warnings,
    })
}

fn serialize_candidate(candidate: &BookCandidate, debug: bool) -> Value {
    let mut value = json!({
        "result_id": candidate.result_id,
        "provider": candidate.provider.as_str(),
        "provider_item_id": candidate.provider_item_id,
        "title": candidate.title,
        "subtitle": candidate.subtitle,
        "authors": candidate.authors,
        "publisher": candidate.publisher,
        "year": candidate.year,
        "language": candidate.language,
        "format": candidate.format,
        "file_size_bytes": candidate.file_size_bytes,
        "cover_url": candidate.cover_url,
        "source_label": candidate.source_label,
        "checksum_md5": candidate.checksum_md5,
        "checksum_sha1": candidate.checksum_sha1,
        "downloadable": candidate.downloadable,
        "relevance_score": candidate.relevance_score,
    });

    if debug {
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "ranking_reasons".to_string(),
                json!(candidate.ranking_reasons),
            );
            object.insert(
                "raw_metadata".to_string(),
                candidate.raw_metadata.clone().unwrap_or(Value::Null),
            );
        }
    }

    value
}

fn serialize_download_result(result: &DownloadResult) -> Value {
    json!({
        "summary": result.summary(),
        "result_id": result.result_id,
        "provider": result.provider.as_str(),
        "provider_item_id": result.provider_item_id,
        "title": result.title,
        "authors": result.authors,
        "format": result.format,
        "mime_type": result.mime_type,
        "destination_path": result.destination_path,
        "final_file_name": result.final_file_name,
        "bytes_written": result.bytes_written,
        "opened": result.opened,
        "warnings": result.warnings,
    })
}

fn search_books_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: "search_books".to_string(),
        description:
            "Search for books and return normalized ranked results with stable result_id values."
                .to_string(),
        input_schema: json_schema_for_object(vec![
            (
                "query",
                string_schema(
                    "Free-text search query. Include title, author, or both.",
                    true,
                ),
            ),
            (
                "formats",
                array_schema(
                    "Optional preferred formats such as pdf, epub, mobi, or azw3.",
                    json!({ "type": "string" }),
                    false,
                ),
            ),
            (
                "categories",
                array_schema(
                    "Optional categories such as fiction or nonfiction.",
                    json!({ "type": "string" }),
                    false,
                ),
            ),
            (
                "languages",
                array_schema(
                    "Optional language filters such as en or fr.",
                    json!({ "type": "string" }),
                    false,
                ),
            ),
            (
                "yearFrom",
                integer_schema("Optional lower publication year bound.", false),
            ),
            (
                "yearTo",
                integer_schema("Optional upper publication year bound.", false),
            ),
            (
                "limit",
                integer_schema("Optional maximum number of results to return.", false),
            ),
            (
                "provider",
                string_schema(
                    "Optional provider hint. Currently annas_archive is supported.",
                    false,
                ),
            ),
            (
                "sort",
                enum_string_schema("Optional sort mode.", &["relevance", "year", "size"], false),
            ),
            (
                "debug",
                boolean_schema(
                    "Include debug ranking and raw provider metadata when available.",
                    false,
                ),
            ),
        ]),
    }
}

fn download_book_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: "download_book".to_string(),
        description: "Download a specific result by result_id or provider_item_id.".to_string(),
        input_schema: json_schema_for_object(vec![
            (
                "resultId",
                string_schema("Stable result_id returned by search_books.", false),
            ),
            (
                "providerItemId",
                string_schema(
                    "Provider-native item identifier as an alternative to result_id.",
                    false,
                ),
            ),
            (
                "downloadDir",
                string_schema("Optional destination directory override.", false),
            ),
            (
                "filenameTemplate",
                string_schema("Optional filename template override.", false),
            ),
            (
                "overwrite",
                boolean_schema("Overwrite an existing destination file if present.", false),
            ),
            (
                "openAfterDownload",
                boolean_schema("Open the file after download completes.", false),
            ),
            (
                "createSubdirs",
                boolean_schema("Create the destination directory if needed.", false),
            ),
        ]),
    }
}

fn search_and_download_book_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: "search_and_download_book".to_string(),
        description:
            "Compatibility workflow that searches, optionally auto-selects, and downloads a book."
                .to_string(),
        input_schema: json_schema_for_object(vec![
            ("query", string_schema("Free-text search query.", true)),
            (
                "formats",
                array_schema(
                    "Optional preferred formats such as pdf, epub, mobi, or azw3.",
                    json!({ "type": "string" }),
                    false,
                ),
            ),
            (
                "categories",
                array_schema(
                    "Optional categories such as fiction or nonfiction.",
                    json!({ "type": "string" }),
                    false,
                ),
            ),
            (
                "languages",
                array_schema(
                    "Optional language filters such as en or fr.",
                    json!({ "type": "string" }),
                    false,
                ),
            ),
            (
                "yearFrom",
                integer_schema("Optional lower publication year bound.", false),
            ),
            (
                "yearTo",
                integer_schema("Optional upper publication year bound.", false),
            ),
            (
                "limit",
                integer_schema("Optional maximum number of results to search.", false),
            ),
            (
                "provider",
                string_schema(
                    "Optional provider hint. Currently annas_archive is supported.",
                    false,
                ),
            ),
            (
                "sort",
                enum_string_schema(
                    "Optional search sort mode.",
                    &["relevance", "year", "size"],
                    false,
                ),
            ),
            (
                "debug",
                boolean_schema(
                    "Include debug ranking and raw provider metadata when available.",
                    false,
                ),
            ),
            (
                "resultId",
                string_schema("Explicit result_id to download after search.", false),
            ),
            (
                "providerItemId",
                string_schema(
                    "Explicit provider-native item id to download after search.",
                    false,
                ),
            ),
            (
                "selectionMode",
                enum_string_schema(
                    "How to select a result when no explicit id is provided.",
                    &["manual", "auto_if_confident", "force_top"],
                    false,
                ),
            ),
            (
                "downloadDir",
                string_schema("Optional destination directory override.", false),
            ),
            (
                "filenameTemplate",
                string_schema("Optional filename template override.", false),
            ),
            (
                "overwrite",
                boolean_schema("Overwrite an existing destination file if present.", false),
            ),
            (
                "openAfterDownload",
                boolean_schema("Open the file after download completes.", false),
            ),
            (
                "createSubdirs",
                boolean_schema("Create the destination directory if needed.", false),
            ),
        ]),
    }
}
