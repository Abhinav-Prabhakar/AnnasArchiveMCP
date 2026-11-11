use std::env;
use std::path::PathBuf;
use std::str::FromStr;

use directories::UserDirs;
use dotenvy::dotenv;

use crate::errors::{AppError, AppResult};

const DEFAULT_PROVIDER: &str = "annas_archive";
const DEFAULT_RAPIDAPI_HOST: &str = "annas-archive-api.p.rapidapi.com";
const DEFAULT_DEFAULT_LIMIT: usize = 10;
const DEFAULT_MAX_LIMIT: usize = 50;
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const DEFAULT_MAX_RETRIES: u32 = 3;
const DEFAULT_USER_AGENT: &str = concat!("annas-archive-mcp/", env!("CARGO_PKG_VERSION"));
const DEFAULT_OPEN_AFTER_DOWNLOAD: bool = false;
const DEFAULT_CREATE_SUBDIRS: bool = true;
const DEFAULT_OVERWRITE: bool = false;
const DEFAULT_FILENAME_TEMPLATE: &str = "{title} - {primary_author}.{ext}";
const DEFAULT_MAX_FILENAME_LENGTH: usize = 180;
const DEFAULT_ENABLE_DEBUG_FIELDS: bool = false;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub provider: ProviderConfig,
    pub rapidapi: RapidApiConfig,
    pub download: DownloadConfig,
    pub search: SearchConfig,
    pub network: NetworkConfig,
    pub logging: LoggingConfig,
    pub server: ServerConfig,
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub default: ProviderKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    AnnasArchive,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AnnasArchive => "annas_archive",
        }
    }
}

impl FromStr for ProviderKind {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "annas_archive" | "annas-archive" | "anna" | "annas" => Ok(Self::AnnasArchive),
            other => Err(AppError::config(format!(
                "unsupported provider `{other}`; supported providers: annas_archive"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RapidApiConfig {
    pub key: Option<String>,
    pub host: String,
}

#[derive(Debug, Clone)]
pub struct DownloadConfig {
    pub default_dir: PathBuf,
    pub open_after_download_default: bool,
    pub create_subdirs: bool,
    pub overwrite_default: bool,
    pub filename_template: String,
    pub max_filename_length: usize,
}

#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub default_limit: usize,
    pub max_limit: usize,
    pub enable_debug_fields: bool,
}

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub user_agent: String,
}

#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub level: String,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub name: String,
    pub version: String,
}

impl AppConfig {
    pub fn load() -> AppResult<Self> {
        let _ = dotenv();

        let provider_default = env_string("ANNAS_ARCHIVE_MCP_DEFAULT_PROVIDER")
            .map(|value| ProviderKind::from_str(&value))
            .transpose()?
            .unwrap_or_else(|| {
                ProviderKind::from_str(DEFAULT_PROVIDER).expect("default provider is valid")
            });

        let rapidapi_key = env_string("RAPIDAPI_KEY");
        let rapidapi_host =
            env_string("RAPIDAPI_HOST").unwrap_or_else(|| DEFAULT_RAPIDAPI_HOST.to_string());

        let default_download_dir = resolve_default_download_dir()?;
        let download_dir =
            env_pathbuf("ANNAS_ARCHIVE_MCP_DOWNLOAD_DIR").unwrap_or(default_download_dir);

        let open_after_download_default = env_bool("ANNAS_ARCHIVE_MCP_OPEN_AFTER_DOWNLOAD")?
            .unwrap_or(DEFAULT_OPEN_AFTER_DOWNLOAD);

        let create_subdirs =
            env_bool("ANNAS_ARCHIVE_MCP_CREATE_SUBDIRS")?.unwrap_or(DEFAULT_CREATE_SUBDIRS);

        let overwrite_default =
            env_bool("ANNAS_ARCHIVE_MCP_OVERWRITE_DEFAULT")?.unwrap_or(DEFAULT_OVERWRITE);

        let filename_template = env_string("ANNAS_ARCHIVE_MCP_FILENAME_TEMPLATE")
            .unwrap_or_else(|| DEFAULT_FILENAME_TEMPLATE.to_string());

        let max_filename_length = env_usize("ANNAS_ARCHIVE_MCP_MAX_FILENAME_LENGTH")?
            .unwrap_or(DEFAULT_MAX_FILENAME_LENGTH);

        let default_limit =
            env_usize("ANNAS_ARCHIVE_MCP_DEFAULT_LIMIT")?.unwrap_or(DEFAULT_DEFAULT_LIMIT);

        let max_limit = env_usize("ANNAS_ARCHIVE_MCP_MAX_LIMIT")?.unwrap_or(DEFAULT_MAX_LIMIT);

        let enable_debug_fields = env_bool("ANNAS_ARCHIVE_MCP_ENABLE_DEBUG_FIELDS")?
            .unwrap_or(DEFAULT_ENABLE_DEBUG_FIELDS);

        let timeout_secs =
            env_u64("ANNAS_ARCHIVE_MCP_TIMEOUT_SECS")?.unwrap_or(DEFAULT_TIMEOUT_SECS);

        let max_retries = env_u32("ANNAS_ARCHIVE_MCP_MAX_RETRIES")?.unwrap_or(DEFAULT_MAX_RETRIES);

        let user_agent = env_string("ANNAS_ARCHIVE_MCP_USER_AGENT")
            .unwrap_or_else(|| DEFAULT_USER_AGENT.to_string());

        let log_level = env_string("RUST_LOG").unwrap_or_else(|| "info".to_string());

        let config = Self {
            provider: ProviderConfig {
                default: provider_default,
            },
            rapidapi: RapidApiConfig {
                key: rapidapi_key,
                host: rapidapi_host,
            },
            download: DownloadConfig {
                default_dir: download_dir,
                open_after_download_default,
                create_subdirs,
                overwrite_default,
                filename_template,
                max_filename_length,
            },
            search: SearchConfig {
                default_limit,
                max_limit,
                enable_debug_fields,
            },
            network: NetworkConfig {
                timeout_secs,
                max_retries,
                user_agent,
            },
            logging: LoggingConfig { level: log_level },
            server: ServerConfig {
                name: "annas-archive-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> AppResult<()> {
        if self.rapidapi.host.trim().is_empty() {
            return Err(AppError::config(
                "RAPIDAPI_HOST must not be empty".to_string(),
            ));
        }

        if self.search.default_limit == 0 {
            return Err(AppError::config(
                "ANNAS_ARCHIVE_MCP_DEFAULT_LIMIT must be greater than 0".to_string(),
            ));
        }

        if self.search.max_limit == 0 {
            return Err(AppError::config(
                "ANNAS_ARCHIVE_MCP_MAX_LIMIT must be greater than 0".to_string(),
            ));
        }

        if self.search.default_limit > self.search.max_limit {
            return Err(AppError::config(format!(
                "ANNAS_ARCHIVE_MCP_DEFAULT_LIMIT ({}) must be less than or equal to ANNAS_ARCHIVE_MCP_MAX_LIMIT ({})",
                self.search.default_limit, self.search.max_limit
            )));
        }

        if self.network.timeout_secs == 0 {
            return Err(AppError::config(
                "ANNAS_ARCHIVE_MCP_TIMEOUT_SECS must be greater than 0".to_string(),
            ));
        }

        if self.download.max_filename_length < 32 {
            return Err(AppError::config(
                "ANNAS_ARCHIVE_MCP_MAX_FILENAME_LENGTH must be at least 32".to_string(),
            ));
        }

        if self.download.filename_template.trim().is_empty() {
            return Err(AppError::config(
                "ANNAS_ARCHIVE_MCP_FILENAME_TEMPLATE must not be empty".to_string(),
            ));
        }

        if self.network.user_agent.trim().is_empty() {
            return Err(AppError::config(
                "ANNAS_ARCHIVE_MCP_USER_AGENT must not be empty".to_string(),
            ));
        }

        Ok(())
    }

    pub fn require_provider_credentials(&self) -> AppResult<()> {
        match self.provider.default {
            ProviderKind::AnnasArchive => {
                if self
                    .rapidapi
                    .key
                    .as_ref()
                    .map(|value| value.trim().is_empty())
                    .unwrap_or(true)
                {
                    return Err(AppError::config(
                        "provider credentials are not configured; set RAPIDAPI_KEY before using this server"
                            .to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    pub fn effective_limit(&self, requested: Option<usize>) -> usize {
        match requested {
            Some(value) if value > 0 => value.min(self.search.max_limit),
            _ => self.search.default_limit,
        }
    }

    pub fn provider_name(&self) -> &'static str {
        self.provider.default.as_str()
    }
}

fn resolve_default_download_dir() -> AppResult<PathBuf> {
    if let Some(user_dirs) = UserDirs::new() {
        if let Some(download_dir) = user_dirs.download_dir() {
            return Ok(download_dir.to_path_buf());
        }

        return Ok(user_dirs.home_dir().join("Downloads"));
    }

    let cwd = env::current_dir().map_err(|error| {
        AppError::config(format!("failed to resolve current directory: {error}"))
    })?;

    Ok(cwd)
}

fn env_string(key: &str) -> Option<String> {
    match env::var(key) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Err(_) => None,
    }
}

fn env_pathbuf(key: &str) -> Option<PathBuf> {
    env_string(key).map(PathBuf::from)
}

fn env_bool(key: &str) -> AppResult<Option<bool>> {
    match env_string(key) {
        Some(value) => Ok(Some(parse_bool(key, &value)?)),
        None => Ok(None),
    }
}

fn env_usize(key: &str) -> AppResult<Option<usize>> {
    match env_string(key) {
        Some(value) => Ok(Some(parse_number::<usize>(key, &value)?)),
        None => Ok(None),
    }
}

fn env_u64(key: &str) -> AppResult<Option<u64>> {
    match env_string(key) {
        Some(value) => Ok(Some(parse_number::<u64>(key, &value)?)),
        None => Ok(None),
    }
}

fn env_u32(key: &str) -> AppResult<Option<u32>> {
    match env_string(key) {
        Some(value) => Ok(Some(parse_number::<u32>(key, &value)?)),
        None => Ok(None),
    }
}

fn parse_bool(key: &str, value: &str) -> AppResult<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" | "on" => Ok(true),
        "0" | "false" | "no" | "n" | "off" => Ok(false),
        _ => Err(AppError::config(format!(
            "invalid boolean value for {key}: `{value}`"
        ))),
    }
}

fn parse_number<T>(key: &str, value: &str) -> AppResult<T>
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    value.parse::<T>().map_err(|error| {
        AppError::config(format!(
            "invalid numeric value for {key}: `{value}` ({error})"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_kind_aliases_work() {
        assert!(matches!(
            ProviderKind::from_str("annas_archive").unwrap(),
            ProviderKind::AnnasArchive
        ));
        assert!(matches!(
            ProviderKind::from_str("annas-archive").unwrap(),
            ProviderKind::AnnasArchive
        ));
        assert!(matches!(
            ProviderKind::from_str("anna").unwrap(),
            ProviderKind::AnnasArchive
        ));
    }

    #[test]
    fn parse_bool_accepts_common_values() {
        assert!(parse_bool("TEST", "true").unwrap());
        assert!(parse_bool("TEST", "1").unwrap());
        assert!(!parse_bool("TEST", "false").unwrap());
        assert!(!parse_bool("TEST", "0").unwrap());
    }

    #[test]
    fn parse_bool_rejects_invalid_values() {
        assert!(parse_bool("TEST", "maybe").is_err());
    }

    #[test]
    fn effective_limit_clamps_to_max() {
        let config = AppConfig {
            provider: ProviderConfig {
                default: ProviderKind::AnnasArchive,
            },
            rapidapi: RapidApiConfig {
                key: None,
                host: DEFAULT_RAPIDAPI_HOST.to_string(),
            },
            download: DownloadConfig {
                default_dir: PathBuf::from("."),
                open_after_download_default: false,
                create_subdirs: true,
                overwrite_default: false,
                filename_template: DEFAULT_FILENAME_TEMPLATE.to_string(),
                max_filename_length: DEFAULT_MAX_FILENAME_LENGTH,
            },
            search: SearchConfig {
                default_limit: 10,
                max_limit: 50,
                enable_debug_fields: false,
            },
            network: NetworkConfig {
                timeout_secs: 30,
                max_retries: 3,
                user_agent: DEFAULT_USER_AGENT.to_string(),
            },
            logging: LoggingConfig {
                level: "info".to_string(),
            },
            server: ServerConfig {
                name: "annas-archive-mcp".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        assert_eq!(config.effective_limit(Some(5)), 5);
        assert_eq!(config.effective_limit(Some(500)), 50);
        assert_eq!(config.effective_limit(Some(0)), 10);
        assert_eq!(config.effective_limit(None), 10);
    }

    #[test]
    fn validate_rejects_bad_limit_ranges() {
        let config = AppConfig {
            provider: ProviderConfig {
                default: ProviderKind::AnnasArchive,
            },
            rapidapi: RapidApiConfig {
                key: None,
                host: DEFAULT_RAPIDAPI_HOST.to_string(),
            },
            download: DownloadConfig {
                default_dir: PathBuf::from("."),
                open_after_download_default: false,
                create_subdirs: true,
                overwrite_default: false,
                filename_template: DEFAULT_FILENAME_TEMPLATE.to_string(),
                max_filename_length: DEFAULT_MAX_FILENAME_LENGTH,
            },
            search: SearchConfig {
                default_limit: 25,
                max_limit: 10,
                enable_debug_fields: false,
            },
            network: NetworkConfig {
                timeout_secs: 30,
                max_retries: 3,
                user_agent: DEFAULT_USER_AGENT.to_string(),
            },
            logging: LoggingConfig {
                level: "info".to_string(),
            },
            server: ServerConfig {
                name: "annas-archive-mcp".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        assert!(config.validate().is_err());
    }
}
