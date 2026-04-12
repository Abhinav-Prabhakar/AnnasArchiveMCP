use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;

use futures_util::TryStreamExt;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::config::{AppConfig, ProviderKind as ConfigProviderKind};
use crate::domain::book::BookCandidate;
use crate::domain::provider::{
    BookProvider, DownloadByteStream, ProviderCapabilities, ProviderDownloadDescriptor,
    ProviderDownloadRequest, ProviderItem, ProviderItemLookupRequest, ProviderKind,
    ProviderSearchRequest, ProviderSearchResponse, SearchSort,
};
use crate::errors::{AppError, ProviderError};

const DEFAULT_BASE_URL: &str = "https://annas-archive-api.p.rapidapi.com";
const SEARCH_ENDPOINT: &str = "search";
const INFO_ENDPOINT: &str = "info";
const DOWNLOAD_ENDPOINT: &str = "download";

#[derive(Clone)]
pub struct AnnasArchiveProvider {
    client: Client,
    base_url: Url,
    api_host: String,
    default_sources: String,
    max_retries: u32,
}

impl AnnasArchiveProvider {
    pub fn new(config: std::sync::Arc<AppConfig>) -> Result<Self, AppError> {
        if !matches!(config.provider.default, ConfigProviderKind::AnnasArchive) {
            return Err(AppError::Provider(ProviderError::RequestFailed {
                reason: "configured provider is not Anna's Archive".to_string(),
            }));
        }

        config.require_provider_credentials()?;

        let api_key = config
            .rapidapi
            .key
            .clone()
            .ok_or(ProviderError::Authentication)?;

        let mut headers = HeaderMap::new();
        headers.insert(
            "x-rapidapi-key",
            HeaderValue::from_str(api_key.trim()).map_err(|err| ProviderError::RequestFailed {
                reason: format!("invalid RapidAPI key header value: {err}"),
            })?,
        );
        headers.insert(
            "x-rapidapi-host",
            HeaderValue::from_str(config.rapidapi.host.trim()).map_err(|err| {
                ProviderError::RequestFailed {
                    reason: format!("invalid RapidAPI host header value: {err}"),
                }
            })?,
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(config.network.user_agent.trim()).map_err(|err| {
                ProviderError::RequestFailed {
                    reason: format!("invalid user agent header value: {err}"),
                }
            })?,
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(config.network.timeout_secs))
            .build()
            .map_err(|err| ProviderError::RequestFailed {
                reason: format!("failed to build HTTP client: {err}"),
            })?;

        let base_url =
            Url::parse(DEFAULT_BASE_URL).map_err(|err| ProviderError::RequestFailed {
                reason: format!("failed to parse provider base URL: {err}"),
            })?;

        Ok(Self {
            client,
            base_url,
            api_host: config.rapidapi.host.clone(),
            default_sources: "libgenLi, libgenRs, zLibrary".to_string(),
            max_retries: config.network.max_retries.max(1),
        })
    }

    fn endpoint_url(&self, endpoint: &str) -> Result<Url, AppError> {
        self.base_url.join(endpoint).map_err(|err| {
            AppError::Provider(ProviderError::RequestFailed {
                reason: format!("failed to build endpoint URL for `{endpoint}`: {err}"),
            })
        })
    }

    async fn search_once(
        &self,
        request: &ProviderSearchRequest,
    ) -> Result<ProviderSearchResponse, AppError> {
        let url = self.endpoint_url(SEARCH_ENDPOINT)?;
        let params = self.search_params(request);

        let response = self
            .client
            .get(url.clone())
            .query(&params)
            .send()
            .await
            .map_err(map_reqwest_error)?;

        Self::map_status(&response)?;
        let body: SearchApiResponse = response.json().await.map_err(|err| {
            AppError::Provider(ProviderError::MalformedResponse {
                reason: format!("failed to decode search response: {err}"),
            })
        })?;

        let items = body
            .hits
            .into_iter()
            .enumerate()
            .filter_map(|(index, hit)| self.normalize_hit(hit, index, request.debug).transpose())
            .collect::<Result<Vec<_>, AppError>>()?;

        Ok(ProviderSearchResponse {
            provider: ProviderKind::AnnasArchive,
            total_returned: items.len(),
            items,
            warnings: Vec::new(),
            debug_metadata: request.debug.then(|| {
                serde_json::json!({
                    "provider": "annas_archive",
                    "api_host": self.api_host,
                    "params": params,
                })
            }),
        })
    }

    fn search_params(&self, request: &ProviderSearchRequest) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("q".to_string(), request.query.clone());
        params.insert("skip".to_string(), "0".to_string());
        params.insert("limit".to_string(), request.limit.to_string());
        params.insert("sort".to_string(), map_sort(request.sort).to_string());

        let categories = if request.categories.is_empty() {
            "fiction, nonfiction".to_string()
        } else {
            request.categories.join(", ")
        };
        params.insert("cat".to_string(), categories);

        let exts = if request.formats.is_empty() {
            "pdf, epub, mobi, azw3".to_string()
        } else {
            request.formats.join(", ")
        };
        params.insert("ext".to_string(), exts);
        params.insert("source".to_string(), self.default_sources.clone());

        params
    }

    fn normalize_hit(
        &self,
        hit: SearchHit,
        index: usize,
        debug: bool,
    ) -> Result<Option<BookCandidate>, AppError> {
        let md5 = match hit.md5.as_deref().map(str::trim) {
            Some(value) if !value.is_empty() => value.to_string(),
            _ => return Ok(None),
        };

        let title = clean_string(hit.title.clone()).unwrap_or_else(|| "Unknown Title".to_string());
        let authors = split_authors(hit.author.clone().as_deref());
        let language = clean_string(hit.language.clone());
        let format = clean_string(hit.extension.clone()).map(|v| v.to_ascii_lowercase());
        let publisher = clean_string(hit.publisher.clone());
        let source_label = clean_string(hit.source.clone());
        let year = clean_string(hit.year.clone()).and_then(|value| value.parse::<u16>().ok());
        let file_size_bytes = hit
            .filesize
            .clone()
            .and_then(|v| (v >= 0).then_some(v as u64));
        let cover_url = clean_string(hit.cover_url.clone());

        let mut ranking_reasons = Vec::new();
        if !authors.is_empty() {
            ranking_reasons.push("author metadata present".to_string());
        }
        if language.is_some() {
            ranking_reasons.push("language metadata present".to_string());
        }
        if format.is_some() {
            ranking_reasons.push("format metadata present".to_string());
        }
        if file_size_bytes.is_some() {
            ranking_reasons.push("file size metadata present".to_string());
        }

        let relevance_score = (100.0_f32 - index as f32).max(1.0);

        Ok(Some(BookCandidate {
            result_id: format!("annas_archive:{}:{}", md5, Uuid::new_v4()),
            provider: crate::domain::book::ProviderKind::AnnasArchive,
            provider_item_id: md5.clone(),
            title,
            subtitle: None,
            authors,
            publisher,
            year,
            language,
            format,
            file_size_bytes,
            cover_url,
            source_label,
            checksum_md5: Some(md5),
            checksum_sha1: None,
            downloadable: true,
            relevance_score,
            ranking_reasons,
            raw_metadata: debug.then(|| serde_json::to_value(hit).ok()).flatten(),
        }))
    }

    async fn lookup_once(
        &self,
        request: &ProviderItemLookupRequest,
    ) -> Result<ProviderItem, AppError> {
        let url = self.endpoint_url(INFO_ENDPOINT)?;
        let response = self
            .client
            .get(url)
            .query(&[("md5", request.provider_item_id.as_str())])
            .send()
            .await
            .map_err(map_reqwest_error)?;

        Self::map_status(&response)?;
        let body: InfoApiResponse = response.json().await.map_err(|err| {
            AppError::Provider(ProviderError::MalformedResponse {
                reason: format!("failed to decode info response: {err}"),
            })
        })?;

        let hit = SearchHit {
            title: body.title,
            author: body.author,
            publisher: body.publisher,
            year: body.year,
            language: body.language,
            extension: body.extension,
            filesize: body.filesize,
            md5: Some(request.provider_item_id.clone()),
            cover_url: body.cover_url,
            source: body.source,
        };

        let candidate = self
            .normalize_hit(hit, 0, request.debug)?
            .ok_or_else(|| AppError::Provider(ProviderError::NotFound))?;

        Ok(ProviderItem {
            candidate,
            available_for_download: true,
            direct_download_url: None,
            warnings: Vec::new(),
            debug_metadata: request.debug.then(|| {
                serde_json::json!({
                    "provider_item_id": request.provider_item_id,
                    "api_host": self.api_host,
                })
            }),
        })
    }

    async fn download_once(
        &self,
        request: &ProviderDownloadRequest,
    ) -> Result<ProviderDownloadDescriptor, AppError> {
        let url = self.endpoint_url(DOWNLOAD_ENDPOINT)?;
        let response = self
            .client
            .get(url)
            .query(&[("md5", request.provider_item_id.as_str())])
            .send()
            .await
            .map_err(map_reqwest_error)?;

        Self::map_status(&response)?;

        let headers = response.headers().clone();
        let content_type = headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());
        let content_length = response.content_length();

        let checksum_md5 = Some(request.provider_item_id.clone());
        let stream: DownloadByteStream = Box::pin(response.bytes_stream().map_err(|err| {
            AppError::Provider(ProviderError::RequestFailed {
                reason: format!("download stream error: {err}"),
            })
        }));

        Ok(ProviderDownloadDescriptor {
            provider: ProviderKind::AnnasArchive,
            provider_item_id: request.provider_item_id.clone(),
            file_name_hint: None,
            content_type,
            content_length,
            checksum_md5,
            checksum_sha1: None,
            headers,
            stream,
        })
    }

    async fn with_retries<F, Fut, T>(&self, mut op: F) -> Result<T, AppError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, AppError>>,
    {
        let mut last_error = None;

        for attempt in 1..=self.max_retries {
            match op().await {
                Ok(value) => return Ok(value),
                Err(err) => {
                    let retryable = matches!(
                        &err,
                        AppError::Provider(ProviderError::Timeout)
                            | AppError::Provider(ProviderError::RateLimited)
                            | AppError::Provider(ProviderError::Unavailable { .. })
                    );

                    if !retryable || attempt == self.max_retries {
                        return Err(err);
                    }

                    last_error = Some(err);
                    tokio::time::sleep(Duration::from_millis(200 * u64::from(attempt))).await;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            AppError::Provider(ProviderError::RequestFailed {
                reason: "provider retry loop exited unexpectedly".to_string(),
            })
        }))
    }

    fn map_status(response: &reqwest::Response) -> Result<(), AppError> {
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }

        let err = match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ProviderError::Authentication,
            StatusCode::TOO_MANY_REQUESTS => ProviderError::RateLimited,
            StatusCode::NOT_FOUND => ProviderError::NotFound,
            s if s.is_server_error() => ProviderError::Unavailable {
                reason: format!("provider returned server error {s}"),
            },
            other => ProviderError::RequestFailed {
                reason: format!("provider returned HTTP {other}"),
            },
        };

        Err(AppError::Provider(err))
    }
}

#[async_trait]
impl BookProvider for AnnasArchiveProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::AnnasArchive
    }

    fn name(&self) -> &'static str {
        "Anna's Archive"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_search: true,
            supports_download: true,
            supports_item_lookup: true,
            supports_streaming_download: true,
            supports_categories: true,
            supports_languages: false,
            supports_format_filtering: true,
            supports_year_filtering: false,
            supports_debug_metadata: true,
        }
    }

    async fn search(
        &self,
        request: ProviderSearchRequest,
    ) -> Result<ProviderSearchResponse, AppError> {
        self.with_retries(|| self.search_once(&request)).await
    }

    async fn get_item(&self, request: ProviderItemLookupRequest) -> Result<ProviderItem, AppError> {
        self.with_retries(|| self.lookup_once(&request)).await
    }

    async fn download(
        &self,
        request: ProviderDownloadRequest,
    ) -> Result<ProviderDownloadDescriptor, AppError> {
        self.with_retries(|| self.download_once(&request)).await
    }
}

fn map_reqwest_error(err: reqwest::Error) -> AppError {
    let provider_error = if err.is_timeout() {
        ProviderError::Timeout
    } else if err.is_connect() {
        ProviderError::Unavailable {
            reason: format!("connection failed: {err}"),
        }
    } else {
        ProviderError::RequestFailed {
            reason: err.to_string(),
        }
    };

    AppError::Provider(provider_error)
}

fn clean_string(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn split_authors(value: Option<&str>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };

    value
        .split(&[',', ';', '&'][..])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn map_sort(sort: SearchSort) -> &'static str {
    match sort {
        SearchSort::Relevance => "mostRelevant",
        SearchSort::Year => "newest",
        SearchSort::Size => "largest",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchApiResponse {
    #[serde(default)]
    hits: Vec<SearchHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchHit {
    title: Option<String>,
    author: Option<String>,
    publisher: Option<String>,
    year: Option<String>,
    language: Option<String>,
    extension: Option<String>,
    filesize: Option<i64>,
    md5: Option<String>,
    #[serde(rename = "cover_url")]
    cover_url: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InfoApiResponse {
    title: Option<String>,
    author: Option<String>,
    publisher: Option<String>,
    year: Option<String>,
    language: Option<String>,
    extension: Option<String>,
    filesize: Option<i64>,
    #[serde(rename = "cover_url")]
    cover_url: Option<String>,
    source: Option<String>,
}
