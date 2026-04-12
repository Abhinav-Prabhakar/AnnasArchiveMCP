use std::borrow::Cow;
use std::path::PathBuf;

use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Configuration(#[from] ConfigurationError),

    #[error("{0}")]
    Validation(#[from] ValidationError),

    #[error("{0}")]
    Provider(#[from] ProviderError),

    #[error("{0}")]
    Download(#[from] DownloadError),

    #[error("{0}")]
    Filesystem(#[from] FilesystemError),

    #[error("{0}")]
    OpenFile(#[from] OpenFileError),

    #[error("{0}")]
    Protocol(#[from] ProtocolError),

    #[error("{0}")]
    Internal(#[from] InternalError),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Configuration(err) => err.code(),
            Self::Validation(err) => err.code(),
            Self::Provider(err) => err.code(),
            Self::Download(err) => err.code(),
            Self::Filesystem(err) => err.code(),
            Self::OpenFile(err) => err.code(),
            Self::Protocol(err) => err.code(),
            Self::Internal(err) => err.code(),
        }
    }

    pub fn user_message(&self) -> Cow<'_, str> {
        match self {
            Self::Configuration(err) => err.user_message(),
            Self::Validation(err) => err.user_message(),
            Self::Provider(err) => err.user_message(),
            Self::Download(err) => err.user_message(),
            Self::Filesystem(err) => err.user_message(),
            Self::OpenFile(err) => err.user_message(),
            Self::Protocol(err) => err.user_message(),
            Self::Internal(err) => err.user_message(),
        }
    }

    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Configuration(_) => false,
            Self::Validation(_) => false,
            Self::Provider(err) => err.is_retryable(),
            Self::Download(err) => err.is_retryable(),
            Self::Filesystem(err) => err.is_retryable(),
            Self::OpenFile(_) => false,
            Self::Protocol(_) => false,
            Self::Internal(_) => false,
        }
    }

    pub fn json_rpc_code(&self) -> i32 {
        match self {
            Self::Protocol(ProtocolError::ParseError { .. }) => -32700,
            Self::Protocol(ProtocolError::InvalidRequest { .. }) => -32600,
            Self::Protocol(ProtocolError::MethodNotFound { .. }) => -32601,
            Self::Protocol(ProtocolError::InvalidParams { .. }) => -32602,
            Self::Validation(_) => -32602,
            Self::Configuration(_)
            | Self::Provider(_)
            | Self::Download(_)
            | Self::Filesystem(_)
            | Self::OpenFile(_)
            | Self::Internal(_) => -32000,
        }
    }

    pub fn as_error_payload(&self) -> ErrorPayload<'_> {
        ErrorPayload {
            code: self.code(),
            message: self.user_message(),
            retryable: self.is_retryable(),
        }
    }

    pub fn config(message: impl Into<String>) -> Self {
        Self::Configuration(ConfigurationError::Invalid {
            key: "application",
            reason: message.into(),
        })
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(ValidationError::Invalid {
            field: "request",
            reason: message.into(),
        })
    }

    pub fn protocol(message: impl Into<String>) -> Self {
        Self::Protocol(ProtocolError::InvalidRequest {
            reason: message.into(),
        })
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::Protocol(ProtocolError::InvalidParams {
            reason: message.into(),
        })
    }

    pub fn method_not_found(message: impl Into<String>) -> Self {
        Self::Protocol(ProtocolError::MethodNotFound {
            method: message.into(),
        })
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(InternalError::Message {
            reason: message.into(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct ErrorPayload<'a> {
    pub code: &'static str,
    pub message: Cow<'a, str>,
    pub retryable: bool,
}

#[derive(Debug, Error)]
pub enum ConfigurationError {
    #[error("Missing required configuration value: {key}")]
    Missing { key: &'static str },

    #[error("Invalid configuration value for {key}: {reason}")]
    Invalid { key: &'static str, reason: String },

    #[error("Unsupported provider configured: {provider}")]
    UnsupportedProvider { provider: String },
}

impl ConfigurationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Missing { .. } => "config_missing",
            Self::Invalid { .. } => "config_invalid",
            Self::UnsupportedProvider { .. } => "config_unsupported_provider",
        }
    }

    pub fn user_message(&self) -> Cow<'_, str> {
        match self {
            Self::Missing { key } if *key == "RAPIDAPI_KEY" => Cow::Borrowed(
                "Provider credentials are not configured. Set RAPIDAPI_KEY before using this server.",
            ),
            _ => Cow::Owned(self.to_string()),
        }
    }
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Query must not be empty.")]
    EmptyQuery,

    #[error("Missing required field: {field}")]
    MissingField { field: &'static str },

    #[error("Invalid value for {field}: {reason}")]
    Invalid { field: &'static str, reason: String },

    #[error("Result id not found: {result_id}")]
    ResultNotFound { result_id: String },

    #[error("Multiple plausible matches were found. Select one using its result_id.")]
    AmbiguousSelection,

    #[error("Requested limit {requested} exceeds maximum allowed {max}")]
    LimitTooLarge { requested: usize, max: usize },

    #[error("Provided path is not allowed: {path}")]
    UnsafePath { path: PathBuf },

    #[error("Unknown tool: {tool}")]
    UnknownTool { tool: String },
}

impl ValidationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::EmptyQuery => "validation_empty_query",
            Self::MissingField { .. } => "validation_missing_field",
            Self::Invalid { .. } => "validation_invalid_field",
            Self::ResultNotFound { .. } => "validation_result_not_found",
            Self::AmbiguousSelection => "validation_ambiguous_selection",
            Self::LimitTooLarge { .. } => "validation_limit_too_large",
            Self::UnsafePath { .. } => "validation_unsafe_path",
            Self::UnknownTool { .. } => "validation_unknown_tool",
        }
    }

    pub fn user_message(&self) -> Cow<'_, str> {
        Cow::Owned(self.to_string())
    }
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("Provider authentication failed.")]
    Authentication,

    #[error("Provider rate limit exceeded. Retry later.")]
    RateLimited,

    #[error("Provider request timed out.")]
    Timeout,

    #[error("Provider resource not found.")]
    NotFound,

    #[error("Provider returned malformed data: {reason}")]
    MalformedResponse { reason: String },

    #[error("Provider is unavailable: {reason}")]
    Unavailable { reason: String },

    #[error("Provider request failed: {reason}")]
    RequestFailed { reason: String },
}

impl ProviderError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Authentication => "provider_authentication_failed",
            Self::RateLimited => "provider_rate_limited",
            Self::Timeout => "provider_timeout",
            Self::NotFound => "provider_not_found",
            Self::MalformedResponse { .. } => "provider_malformed_response",
            Self::Unavailable { .. } => "provider_unavailable",
            Self::RequestFailed { .. } => "provider_request_failed",
        }
    }

    pub fn user_message(&self) -> Cow<'_, str> {
        match self {
            Self::Authentication => Cow::Borrowed(
                "Provider authentication failed. Check your API credentials and try again.",
            ),
            Self::RateLimited => Cow::Borrowed(
                "The upstream provider is rate-limiting requests. Please retry later.",
            ),
            Self::Timeout => {
                Cow::Borrowed("The upstream provider timed out before the request completed.")
            }
            Self::NotFound => Cow::Borrowed("The requested provider resource was not found."),
            _ => Cow::Owned(self.to_string()),
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited | Self::Timeout | Self::Unavailable { .. }
        )
    }
}

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("No downloadable file is available for the selected result.")]
    NotDownloadable,

    #[error("Download failed after {attempts} attempt(s): {reason}")]
    Failed { attempts: usize, reason: String },

    #[error("Download was interrupted before completion.")]
    Interrupted,

    #[error("Downloaded file did not pass integrity checks: {reason}")]
    IntegrityMismatch { reason: String },

    #[error("Downloaded file type could not be determined safely.")]
    UnknownFileType,
}

impl DownloadError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotDownloadable => "download_not_downloadable",
            Self::Failed { .. } => "download_failed",
            Self::Interrupted => "download_interrupted",
            Self::IntegrityMismatch { .. } => "download_integrity_mismatch",
            Self::UnknownFileType => "download_unknown_file_type",
        }
    }

    pub fn user_message(&self) -> Cow<'_, str> {
        Cow::Owned(self.to_string())
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Failed { .. } | Self::Interrupted)
    }
}

#[derive(Debug, Error)]
pub enum FilesystemError {
    #[error("Could not create directory: {path}")]
    CreateDir { path: PathBuf },

    #[error("Could not write file: {path}")]
    WriteFile { path: PathBuf },

    #[error("Could not move temporary file into place: {from} -> {to}")]
    AtomicRename { from: PathBuf, to: PathBuf },

    #[error("The destination file already exists and overwrite is disabled: {path}")]
    AlreadyExists { path: PathBuf },

    #[error("Resolved destination is outside the allowed directory: {path}")]
    PathEscape { path: PathBuf },

    #[error("Filesystem operation failed: {reason}")]
    Io { reason: String },
}

impl FilesystemError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::CreateDir { .. } => "filesystem_create_dir_failed",
            Self::WriteFile { .. } => "filesystem_write_failed",
            Self::AtomicRename { .. } => "filesystem_atomic_rename_failed",
            Self::AlreadyExists { .. } => "filesystem_already_exists",
            Self::PathEscape { .. } => "filesystem_path_escape",
            Self::Io { .. } => "filesystem_io_error",
        }
    }

    pub fn user_message(&self) -> Cow<'_, str> {
        Cow::Owned(self.to_string())
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Io { .. })
    }
}

#[derive(Debug, Error)]
pub enum OpenFileError {
    #[error("Opening downloaded files is disabled by configuration.")]
    Disabled,

    #[error("Failed to open downloaded file: {path}")]
    Failed { path: PathBuf },
}

impl OpenFileError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Disabled => "open_file_disabled",
            Self::Failed { .. } => "open_file_failed",
        }
    }

    pub fn user_message(&self) -> Cow<'_, str> {
        Cow::Owned(self.to_string())
    }
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Failed to parse protocol message: {reason}")]
    ParseError { reason: String },

    #[error("Invalid protocol request: {reason}")]
    InvalidRequest { reason: String },

    #[error("Method not found: {method}")]
    MethodNotFound { method: String },

    #[error("Invalid parameters: {reason}")]
    InvalidParams { reason: String },
}

impl ProtocolError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ParseError { .. } => "protocol_parse_error",
            Self::InvalidRequest { .. } => "protocol_invalid_request",
            Self::MethodNotFound { .. } => "protocol_method_not_found",
            Self::InvalidParams { .. } => "protocol_invalid_params",
        }
    }

    pub fn user_message(&self) -> Cow<'_, str> {
        Cow::Owned(self.to_string())
    }
}

#[derive(Debug, Error)]
pub enum InternalError {
    #[error("Internal error: {reason}")]
    Message { reason: String },

    #[error("Unexpected internal state: {reason}")]
    UnexpectedState { reason: String },
}

impl InternalError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Message { .. } => "internal_error",
            Self::UnexpectedState { .. } => "internal_unexpected_state",
        }
    }

    pub fn user_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("An internal error occurred while processing the request.")
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Filesystem(FilesystemError::Io {
            reason: value.to_string(),
        })
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::Protocol(ProtocolError::ParseError {
            reason: value.to_string(),
        })
    }
}
