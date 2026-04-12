use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::StreamExt;
use infer::Infer;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

use crate::config::AppConfig;
use crate::domain::book::BookCandidate;
use crate::domain::download::{DownloadRequest, DownloadResult};
use crate::domain::provider::{BookProvider, ProviderDownloadRequest, ProviderKind};
use crate::errors::{AppError, DownloadError, FilesystemError, ValidationError};
use crate::services::open_service::OpenService;
use crate::util::filenames::{
    build_download_filename, ensure_unique_path, normalize_extension, reconcile_extension,
};
use crate::util::formatting::format_bytes;
use crate::util::paths::{ensure_path_within_root, join_filename_safely, resolve_download_dir};

pub struct DownloadService<P>
where
    P: BookProvider,
{
    provider: Arc<P>,
    open_service: Arc<OpenService>,
    config: Arc<AppConfig>,
    infer: Infer,
}

impl<P> DownloadService<P>
where
    P: BookProvider,
{
    pub fn new(provider: Arc<P>, open_service: Arc<OpenService>, config: Arc<AppConfig>) -> Self {
        Self {
            provider,
            open_service,
            config,
            infer: Infer::new(),
        }
    }

    pub async fn download(
        &self,
        candidate: &BookCandidate,
        request: DownloadRequest,
    ) -> Result<DownloadResult, AppError> {
        self.validate_candidate(candidate)?;
        request.validate().map_err(|err| ValidationError::Invalid {
            field: "download_request",
            reason: format!("{err:?}"),
        })?;

        let requested_root = request
            .download_dir
            .clone()
            .unwrap_or_else(|| self.config.download.default_dir.clone());

        let root_dir = resolve_download_dir(Some(requested_root.as_path()))?;
        if request.create_subdirs {
            fs::create_dir_all(&root_dir)
                .await
                .map_err(|_| FilesystemError::CreateDir {
                    path: root_dir.clone(),
                })?;
        }

        let preferred_extension = request
            .filename_template
            .as_ref()
            .and_then(|_| candidate.format.clone())
            .or_else(|| candidate.format.clone());

        let planned_filename = build_download_filename(
            Some(candidate.title.as_str()),
            candidate.primary_author(),
            preferred_extension.as_deref(),
        );

        let final_path =
            self.resolve_output_path(&root_dir, &planned_filename, request.overwrite)?;

        let temp_path = temp_path_for(&final_path);
        let provider_request = ProviderDownloadRequest {
            provider_item_id: candidate.provider_item_id.clone(),
            debug: false,
        };

        info!(
            provider = %self.provider.name(),
            result_id = %candidate.result_id,
            provider_item_id = %candidate.provider_item_id,
            destination = %final_path.display(),
            "starting book download"
        );

        let descriptor = self.provider.download(provider_request).await?;

        let mut file = File::create(&temp_path)
            .await
            .map_err(|_| FilesystemError::WriteFile {
                path: temp_path.clone(),
            })?;

        let mut sniffed_prefix = Vec::with_capacity(8192);
        let mut bytes_written: u64 = 0;
        let mut stream = descriptor.stream;
        let mut warnings = Vec::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            if sniffed_prefix.len() < 8192 {
                let remaining = 8192usize.saturating_sub(sniffed_prefix.len());
                sniffed_prefix.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
            }

            file.write_all(&chunk)
                .await
                .map_err(|_| FilesystemError::WriteFile {
                    path: temp_path.clone(),
                })?;
            bytes_written += chunk.len() as u64;
        }

        file.flush().await.map_err(|_| FilesystemError::WriteFile {
            path: temp_path.clone(),
        })?;
        file.sync_all()
            .await
            .map_err(|_| FilesystemError::WriteFile {
                path: temp_path.clone(),
            })?;
        drop(file);

        if bytes_written == 0 {
            let _ = fs::remove_file(&temp_path).await;
            return Err(DownloadError::Failed {
                attempts: 1,
                reason: "provider returned an empty file".to_string(),
            }
            .into());
        }

        let detected = self.detect_file_type(&sniffed_prefix, descriptor.content_type.as_deref());

        let current_filename = final_path
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("download.bin");

        let current_stem = Path::new(current_filename)
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("download");

        let reconciled_extension = reconcile_extension(
            Path::new(current_filename)
                .extension()
                .and_then(|v| v.to_str()),
            descriptor
                .file_name_hint
                .as_deref()
                .and_then(|name| Path::new(name).extension().and_then(|v| v.to_str())),
            detected.extension.as_deref(),
        )
        .unwrap_or_else(|| "bin".to_string());

        let reconciled_filename = format!("{current_stem}.{reconciled_extension}");

        let committed_path = self
            .commit_temp_file(
                &root_dir,
                &final_path,
                &temp_path,
                &reconciled_filename,
                request.overwrite,
            )
            .await?;

        if let (Some(reported), Some(detected_mime)) = (
            descriptor.content_type.as_deref(),
            detected.mime_type.as_deref(),
        ) {
            if !reported.eq_ignore_ascii_case(detected_mime) {
                warnings.push(format!(
                    "Provider reported content type `{reported}`, but file signature suggests `{detected_mime}`."
                ));
            }
        }

        let opened = if request.open_after_download {
            match self.open_service.open(&committed_path).await {
                Ok(()) => true,
                Err(err) => {
                    warn!(
                        path = %committed_path.display(),
                        error = %err,
                        "failed to open downloaded file"
                    );
                    warnings.push(format!(
                        "Downloaded successfully, but opening the file failed: {err}"
                    ));
                    false
                }
            }
        } else {
            false
        };

        debug!(
            path = %committed_path.display(),
            size = %format_bytes(bytes_written),
            mime_type = ?detected.mime_type,
            "download completed"
        );

        Ok(DownloadResult {
            result_id: Some(candidate.result_id.clone()),
            provider: map_provider_kind(candidate.provider),
            provider_item_id: candidate.provider_item_id.clone(),
            title: candidate.title.clone(),
            authors: candidate.authors.clone(),
            format: normalize_extension(committed_path.extension().and_then(|v| v.to_str())),
            mime_type: detected.mime_type.or(descriptor.content_type),
            destination_path: committed_path.clone(),
            final_file_name: committed_path
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("download.bin")
                .to_string(),
            bytes_written,
            opened,
            warnings,
        })
    }

    fn validate_candidate(&self, candidate: &BookCandidate) -> Result<(), AppError> {
        if !candidate.downloadable {
            return Err(DownloadError::NotDownloadable.into());
        }

        if candidate.provider_item_id.trim().is_empty() {
            return Err(ValidationError::Invalid {
                field: "provider_item_id",
                reason: "selected result does not contain a valid provider item id".to_string(),
            }
            .into());
        }

        Ok(())
    }

    fn resolve_output_path(
        &self,
        root_dir: &Path,
        desired_filename: &str,
        overwrite: bool,
    ) -> Result<PathBuf, AppError> {
        let desired_path = join_filename_safely(root_dir, desired_filename)?;

        if overwrite {
            return Ok(desired_path);
        }

        Ok(ensure_unique_path(root_dir, desired_filename))
    }

    async fn commit_temp_file(
        &self,
        root_dir: &Path,
        planned_path: &Path,
        temp_path: &Path,
        reconciled_filename: &str,
        overwrite: bool,
    ) -> Result<PathBuf, AppError> {
        let final_path = join_filename_safely(root_dir, reconciled_filename)?;
        ensure_path_within_root(root_dir, &final_path)?;

        if final_path.exists() && !overwrite {
            let _ = fs::remove_file(temp_path).await;
            return Err(FilesystemError::AlreadyExists {
                path: final_path.clone(),
            }
            .into());
        }

        if final_path != planned_path && planned_path.exists() && overwrite {
            let _ = fs::remove_file(planned_path).await;
        }

        if final_path.exists() && overwrite {
            fs::remove_file(&final_path)
                .await
                .map_err(|_| FilesystemError::Io {
                    reason: format!(
                        "failed to remove existing destination before overwrite: {}",
                        final_path.display()
                    ),
                })?;
        }

        fs::rename(temp_path, &final_path)
            .await
            .map_err(|_| FilesystemError::AtomicRename {
                from: temp_path.to_path_buf(),
                to: final_path.clone(),
            })?;

        Ok(final_path)
    }

    fn detect_file_type(
        &self,
        sniffed_prefix: &[u8],
        reported_content_type: Option<&str>,
    ) -> DetectedFileType {
        if sniffed_prefix.is_empty() {
            return DetectedFileType {
                mime_type: reported_content_type.map(ToOwned::to_owned),
                extension: None,
            };
        }

        if let Some(kind) = self.infer.get(sniffed_prefix) {
            return DetectedFileType {
                mime_type: Some(kind.mime_type().to_string()),
                extension: Some(kind.extension().to_string()),
            };
        }

        DetectedFileType {
            mime_type: reported_content_type.map(ToOwned::to_owned),
            extension: None,
        }
    }
}

struct DetectedFileType {
    mime_type: Option<String>,
    extension: Option<String>,
}

fn temp_path_for(final_path: &Path) -> PathBuf {
    let file_name = final_path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("download.bin");
    final_path.with_file_name(format!("{file_name}.part"))
}

fn map_provider_kind(kind: crate::domain::book::ProviderKind) -> ProviderKind {
    match kind {
        crate::domain::book::ProviderKind::AnnasArchive => ProviderKind::AnnasArchive,
        crate::domain::book::ProviderKind::Unknown => ProviderKind::AnnasArchive,
    }
}
