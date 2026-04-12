use crate::config::LoggingConfig;
use crate::errors::AppResult;
use tracing_subscriber::{fmt, EnvFilter};

/// Initialize structured logging for the application.
///
/// Important behavior:
/// - logs are written to stderr so stdout remains clean for MCP protocol traffic
/// - the configured log level is used as a fallback if `RUST_LOG` is not set
/// - repeated initialization is treated as a harmless no-op
pub fn init(config: &LoggingConfig) -> AppResult<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = if config.level.trim().is_empty() {
            "info"
        } else {
            config.level.trim()
        };

        EnvFilter::new(level)
    });

    let subscriber = fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .with_target(true)
        .with_line_number(false)
        .with_file(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_ansi(false)
        .finish();

    match tracing::subscriber::set_global_default(subscriber) {
        Ok(()) => Ok(()),
        Err(_) => Ok(()),
    }
}
