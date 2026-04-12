use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::domain::provider::ProviderKind;

/// How a download target should be selected.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelectionMode {
    /// Select by the server-generated stable result id.
    ResultId,
    /// Select by the upstream provider's native item id.
    ProviderItemId,
    /// Choose the top-ranked result only if confidence is high enough.
    AutoIfConfident,
    /// Always choose the top-ranked result.
    ForceTop,
    /// Compatibility mode for legacy index-based selection.
    LegacyByIndex,
}

impl Default for SelectionMode {
    fn default() -> Self {
        Self::ResultId
    }
}

/// Request describing how a book should be downloaded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadRequest {
    pub selection_mode: SelectionMode,
    pub result_id: Option<String>,
    pub provider_item_id: Option<String>,
    pub legacy_index: Option<usize>,
    pub download_dir: Option<PathBuf>,
    pub filename_template: Option<String>,
    pub overwrite: bool,
    pub create_subdirs: bool,
    pub open_after_download: bool,
}

impl Default for DownloadRequest {
    fn default() -> Self {
        Self {
            selection_mode: SelectionMode::ResultId,
            result_id: None,
            provider_item_id: None,
            legacy_index: None,
            download_dir: None,
            filename_template: None,
            overwrite: false,
            create_subdirs: true,
            open_after_download: false,
        }
    }
}

impl DownloadRequest {
    pub fn with_result_id(result_id: impl Into<String>) -> Self {
        Self {
            result_id: Some(result_id.into()),
            ..Self::default()
        }
    }

    pub fn with_provider_item_id(provider_item_id: impl Into<String>) -> Self {
        Self {
            selection_mode: SelectionMode::ProviderItemId,
            provider_item_id: Some(provider_item_id.into()),
            ..Self::default()
        }
    }

    pub fn legacy_by_index(index: usize) -> Self {
        Self {
            selection_mode: SelectionMode::LegacyByIndex,
            legacy_index: Some(index),
            ..Self::default()
        }
    }

    pub fn validate(&self) -> Result<(), DownloadRequestValidationError> {
        match self.selection_mode {
            SelectionMode::ResultId => {
                if self
                    .result_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .is_none()
                {
                    return Err(DownloadRequestValidationError::MissingResultId);
                }
            }
            SelectionMode::ProviderItemId => {
                if self
                    .provider_item_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .is_none()
                {
                    return Err(DownloadRequestValidationError::MissingProviderItemId);
                }
            }
            SelectionMode::LegacyByIndex => {
                if self.legacy_index.is_none() {
                    return Err(DownloadRequestValidationError::MissingLegacyIndex);
                }
            }
            SelectionMode::AutoIfConfident | SelectionMode::ForceTop => {}
        }

        if let Some(template) = &self.filename_template {
            if template.trim().is_empty() {
                return Err(DownloadRequestValidationError::EmptyFilenameTemplate);
            }
        }

        Ok(())
    }
}

/// Normalized description of the item selected for download.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadTarget {
    pub provider: ProviderKind,
    pub provider_item_id: String,
    pub result_id: Option<String>,
    pub title: String,
    pub authors: Vec<String>,
    pub format: Option<String>,
    pub mime_type: Option<String>,
    pub checksum_md5: Option<String>,
    pub checksum_sha1: Option<String>,
}

/// Information about the file written to disk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadedFile {
    pub destination_path: PathBuf,
    pub final_file_name: String,
    pub bytes_written: u64,
    pub mime_type: Option<String>,
    pub detected_extension: Option<String>,
    pub opened: bool,
}

/// Final normalized result returned after a successful download.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadResult {
    pub result_id: Option<String>,
    pub provider: ProviderKind,
    pub provider_item_id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub format: Option<String>,
    pub mime_type: Option<String>,
    pub destination_path: PathBuf,
    pub final_file_name: String,
    pub bytes_written: u64,
    pub opened: bool,
    pub warnings: Vec<String>,
}

impl DownloadResult {
    pub fn from_parts(target: DownloadTarget, file: DownloadedFile, warnings: Vec<String>) -> Self {
        Self {
            result_id: target.result_id,
            provider: target.provider,
            provider_item_id: target.provider_item_id,
            title: target.title,
            authors: target.authors,
            format: target.format,
            mime_type: file.mime_type.or(target.mime_type),
            destination_path: file.destination_path,
            final_file_name: file.final_file_name,
            bytes_written: file.bytes_written,
            opened: file.opened,
            warnings,
        }
    }

    pub fn summary(&self) -> String {
        let author_text = if self.authors.is_empty() {
            "Unknown author".to_string()
        } else {
            self.authors.join(", ")
        };

        format!(
            "Downloaded \"{}\" by {} to {}",
            self.title,
            author_text,
            self.destination_path.display()
        )
    }
}

/// Validation failures for [`DownloadRequest`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadRequestValidationError {
    MissingResultId,
    MissingProviderItemId,
    MissingLegacyIndex,
    EmptyFilenameTemplate,
}
