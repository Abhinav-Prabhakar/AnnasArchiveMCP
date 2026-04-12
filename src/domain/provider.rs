use std::fmt;
use std::pin::Pin;

use async_trait::async_trait;
use bytes::Bytes;
use futures_core::Stream;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::domain::book::BookCandidate;
use crate::errors::AppError;

/// Boxed async byte stream used for streamed provider downloads.
pub type DownloadByteStream =
    Pin<Box<dyn Stream<Item = Result<Bytes, AppError>> + Send + Sync + 'static>>;

/// Supported upstream provider kinds.
///
/// Keep this enum small and stable in the public domain layer.
/// Provider-specific quirks should stay in provider implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    AnnasArchive,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AnnasArchive => "annas_archive",
        }
    }
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Capability flags describing what a provider supports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub supports_search: bool,
    pub supports_download: bool,
    pub supports_item_lookup: bool,
    pub supports_streaming_download: bool,
    pub supports_categories: bool,
    pub supports_languages: bool,
    pub supports_format_filtering: bool,
    pub supports_year_filtering: bool,
    pub supports_debug_metadata: bool,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            supports_search: true,
            supports_download: true,
            supports_item_lookup: true,
            supports_streaming_download: true,
            supports_categories: true,
            supports_languages: true,
            supports_format_filtering: true,
            supports_year_filtering: true,
            supports_debug_metadata: true,
        }
    }
}

/// Normalized provider search request used by the service layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSearchRequest {
    pub query: String,
    pub formats: Vec<String>,
    pub categories: Vec<String>,
    pub languages: Vec<String>,
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    pub limit: u32,
    pub sort: SearchSort,
    pub debug: bool,
}

impl ProviderSearchRequest {
    pub fn is_empty_query(&self) -> bool {
        self.query.trim().is_empty()
    }
}

/// Supported search sort modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSort {
    Relevance,
    Year,
    Size,
}

/// Optional provider item lookup request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderItemLookupRequest {
    pub provider_item_id: String,
    pub debug: bool,
}

/// Provider-side item representation after optional enrichment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderItem {
    pub candidate: BookCandidate,
    pub available_for_download: bool,
    pub direct_download_url: Option<Url>,
    pub warnings: Vec<String>,
    pub debug_metadata: Option<serde_json::Value>,
}

/// Search response returned by a provider before higher-level formatting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderSearchResponse {
    pub provider: ProviderKind,
    pub items: Vec<BookCandidate>,
    pub total_returned: usize,
    pub warnings: Vec<String>,
    pub debug_metadata: Option<serde_json::Value>,
}

/// Request to begin downloading a provider item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderDownloadRequest {
    pub provider_item_id: String,
    pub debug: bool,
}

/// Download descriptor returned by a provider when a download stream is opened.
pub struct ProviderDownloadDescriptor {
    pub provider: ProviderKind,
    pub provider_item_id: String,
    pub file_name_hint: Option<String>,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub checksum_md5: Option<String>,
    pub checksum_sha1: Option<String>,
    pub headers: HeaderMap,
    pub stream: DownloadByteStream,
}

/// Shared provider interface.
///
/// The application layer should depend on this trait instead of any concrete
/// provider implementation.
#[async_trait]
pub trait BookProvider: Send + Sync {
    /// Stable provider kind identifier.
    fn kind(&self) -> ProviderKind;

    /// Human-friendly provider name for logs and summaries.
    fn name(&self) -> &'static str;

    /// Capability flags describing this provider.
    fn capabilities(&self) -> ProviderCapabilities;

    /// Execute a search against the upstream provider.
    async fn search(
        &self,
        request: ProviderSearchRequest,
    ) -> Result<ProviderSearchResponse, AppError>;

    /// Fetch a single provider item with richer metadata when available.
    async fn get_item(&self, request: ProviderItemLookupRequest) -> Result<ProviderItem, AppError>;

    /// Begin a streamed download for a provider item.
    async fn download(
        &self,
        request: ProviderDownloadRequest,
    ) -> Result<ProviderDownloadDescriptor, AppError>;
}
